mod cache;

use anyhow::{anyhow, Result};
use async_openai::{
    types::chat::{
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
        CompletionUsage, CreateChatCompletionRequestArgs,
    },
    Client,
};
use clap::Parser;
use glob::Pattern;
use serde::Deserialize;
use std::fs;
use std::io::{self, Write};
use std::process::Command;
use std::str;

fn format_thousands(n: usize) -> String {
    let s = n.to_string();
    let len = s.len();
    let mut result = String::with_capacity(len + (len - 1) / 3);
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result
}

const SYSTEM_PROMPT: &str = r#"You are an expert software engineer that generates concise, one-line Git commit messages based on the provided diffs.
Review the provided context and diffs which are about to be committed to a git repo.
Review the diffs carefully.
Generate a one-line commit message for those changes.
The commit message should be structured as follows: <type>: <description>
Use these for <type>: fix, feat, build, chore, ci, docs, style, refactor, perf, test

Ensure the commit message:{language_instruction}
- Is in the imperative mood (e.g., "add feature" not "added feature" or "adding feature").
- Does not exceed 72 characters.

Reply only with the one-line commit message, without any additional text, explanations, or line breaks."#;

#[derive(Debug, Deserialize)]
struct Config {
    openai_base_url: Option<String>,
    openai_api_key: Option<String>,
    #[serde(default = "default_model")]
    model: String,
    language: Option<String>,
    #[serde(default = "default_ignore")]
    ignore: Vec<String>,
}

fn default_model() -> String {
    "deepseek-v3-1".to_string()
}

fn default_ignore() -> Vec<String> {
    vec![
        "*lock*".to_string(),
        "*.log".to_string(),
        "target/".to_string(),
        "dist/".to_string(),
        "build/".to_string(),
    ]
}

impl Default for Config {
    fn default() -> Self {
        Config {
            openai_base_url: None,
            openai_api_key: None,
            model: default_model(),
            language: None,
            ignore: default_ignore(),
        }
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Do not run git commit hooks
    #[arg(long)]
    no_verify: bool,
}

fn load_config() -> Result<Config> {
    let config_path = dirs::home_dir()
        .ok_or_else(|| anyhow!("Failed to find home directory"))?
        .join(".ai-commit-rs.toml");

    if !config_path.exists() {
        return Ok(Config::default());
    }

    let config_str = fs::read_to_string(config_path)?;
    let config: Config = toml::from_str(&config_str)?;
    Ok(config)
}

fn log(message: &str) {
    if std::env::var("AI_COMMIT_DEBUG").is_ok() {
        println!("[DEBUG] {}", message);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = load_config()?;

    if let Some(base_url) = &config.openai_base_url {
        std::env::set_var("OPENAI_BASE_URL", base_url);
    }
    if let Some(api_key) = &config.openai_api_key {
        std::env::set_var("OPENAI_API_KEY", api_key);
    }

    run(&config, cli.no_verify).await
}

async fn run(config: &Config, no_verify: bool) -> Result<()> {
    let git_root_output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()?;

    if !git_root_output.status.success() {
        return Err(anyhow!(
            "Not a git repository or git not found: {}",
            String::from_utf8_lossy(&git_root_output.stderr)
        ));
    }

    let git_root = String::from_utf8(git_root_output.stdout)?
        .trim()
        .to_string();

    let ignore_patterns = config
        .ignore
        .iter()
        .map(|s| Pattern::new(s))
        .collect::<Result<Vec<_>, _>>()?;

    let diffs = get_staged_diffs(&git_root, Some(&ignore_patterns))?;

    if diffs.is_empty() {
        println!("No staged changes to commit after filtering.");
        return Ok(());
    }

    let model = &config.model;
    let language = config.language.as_deref();

    // Only compute hash when cache exists (to compare) or on failure (to save)
    let cached = cache::load();
    let diff_hash = cached.as_ref().map(|_| cache::compute_diff_hash(&diffs));

    let (commit_msg, usage) = match (&cached, &diff_hash) {
        (Some(c), Some(hash)) if c.hash == *hash => {
            println!("Cache hit, skipping LLM.");
            (c.message.clone(), None)
        }
        _ => {
            let client = Client::new();
            let (msg, usage) = generate_commit_message(&client, &diffs, model, language).await?;
            (msg, usage)
        }
    };

    if let Some(usage) = usage {
        println!(
            "Tokens used: {}, input tokens: {}, output tokens: {}",
            usage.total_tokens, usage.prompt_tokens, usage.completion_tokens
        );
    }

    match perform_commit(&git_root, &commit_msg, no_verify) {
        Ok((commit_hash, commit_message)) => {
            println!("Commit {} {}", commit_hash, commit_message);
            cache::clear();
        }
        Err(e) => {
            let hash = diff_hash.unwrap_or_else(|| cache::compute_diff_hash(&diffs));
            cache::save(&hash, &commit_msg)?;
            println!("Commit failed, cached message for next attempt.");
            return Err(e);
        }
    }

    Ok(())
}

fn get_staged_diffs(git_root: &str, filter_patterns: Option<&Vec<Pattern>>) -> Result<String> {
    log("Executing: git diff --staged --name-only -z");
    let output = Command::new("git")
        .current_dir(git_root)
        .args(["diff", "--staged", "--name-only", "-z"])
        .output()?;
    if !output.status.success() {
        return Err(anyhow!(
            "Failed to get staged files: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = str::from_utf8(&output.stdout)?;
    let staged_files: Vec<&str> = stdout.split('\0').filter(|s| !s.is_empty()).collect();
    log(&format!("Found staged files: {:?}", staged_files));

    let filtered_files: Vec<&str> = staged_files
        .into_iter()
        .filter(|file| {
            if let Some(patterns) = filter_patterns {
                let matched = patterns.iter().any(|p| p.matches(file));
                if matched {
                    log(&format!("Ignoring file matching pattern: {}", file));
                }
                !matched
            } else {
                true
            }
        })
        .collect();

    log(&format!("Files after filtering: {:?}", filtered_files));

    if filtered_files.is_empty() {
        return Ok(String::new());
    }

    let mut diff_command = Command::new("git");
    diff_command.current_dir(git_root);
    diff_command.arg("diff").arg("--staged").arg("--");
    diff_command.args(&filtered_files);

    log(&format!("Executing: {:?}", diff_command));

    let diff_output = diff_command.output()?;

    if !diff_output.status.success() {
        return Err(anyhow!(
            "Failed to get staged diffs: {}",
            String::from_utf8_lossy(&diff_output.stderr)
        ));
    }

    let diff_content = String::from_utf8(diff_output.stdout).map_err(|e| anyhow!(e))?;

    if diff_content.trim().is_empty() && !filtered_files.is_empty() {
        return Ok(format!("Staged files:\n{}", filtered_files.join("\n")));
    }

    Ok(diff_content)
}
async fn generate_commit_message(
    client: &Client<impl async_openai::config::Config>,
    diffs: &str,
    model: &str,
    language: Option<&str>,
) -> Result<(String, Option<CompletionUsage>)> {
    let user_content = format!("# Diffs:\n{}", diffs);

    const MAX_TOKENS_THRESHOLD: usize = 4000;
    let approx_tokens = user_content.len() / 4;

    if approx_tokens > MAX_TOKENS_THRESHOLD {
        print!(
            "The staged diff is large (approximately {} tokens). It may consume a lot of tokens and take a long time. Do you want to continue? (y/N) ",
            format_thousands(approx_tokens)
        );
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            return Err(anyhow!("Aborted by user."));
        }
    }

    let language_instruction = if let Some(lang) = language {
        format!("\n- Is written in {}.", lang)
    } else {
        String::new()
    };

    let system_prompt = SYSTEM_PROMPT.replace("{language_instruction}", &language_instruction);

    let request = CreateChatCompletionRequestArgs::default()
        .model(model)
        .messages([
            ChatCompletionRequestSystemMessageArgs::default()
                .content(system_prompt)
                .build()?
                .into(),
            ChatCompletionRequestUserMessageArgs::default()
                .content(user_content)
                .build()?
                .into(),
        ])
        .build()?;

    println!("Waiting for {} to generate commit message...", model);
    let response = client.chat().create(request).await?;

    if let Some(choice) = response.choices.into_iter().next() {
        let message = choice.message.content.unwrap_or_default();
        Ok((message.trim().trim_matches('"').to_string(), response.usage))
    } else {
        Err(anyhow!("Failed to get commit message from LLM"))
    }
}

fn perform_commit(git_root: &str, message: &str, no_verify: bool) -> Result<(String, String)> {
    let mut command_args = vec!["commit"];
    if no_verify {
        command_args.push("--no-verify");
    }
    command_args.extend_from_slice(&["-m", message]);

    let output = Command::new("git")
        .current_dir(git_root)
        .args(command_args)
        .output()?;

    if !output.status.success() {
        return Err(anyhow!(
            "Failed to commit: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let rev_parse_output = Command::new("git")
        .current_dir(git_root)
        .args(["rev-parse", "--short", "HEAD"])
        .output()?;

    if !rev_parse_output.status.success() {
        return Err(anyhow!(
            "Failed to get commit hash: {}",
            String::from_utf8_lossy(&rev_parse_output.stderr)
        ));
    }

    let short_hash = String::from_utf8(rev_parse_output.stdout)?
        .trim()
        .to_string();

    Ok((
        short_hash,
        message.lines().next().unwrap_or_default().to_string(),
    ))
}
