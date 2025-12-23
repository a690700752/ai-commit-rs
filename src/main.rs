use anyhow::{anyhow, Result};
use async_openai::{
    types::chat::{
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
        CompletionUsage, CreateChatCompletionRequestArgs,
    },
    Client,
};
use glob::Pattern;
use serde::Deserialize;
use std::fs;
use std::process::Command;
use std::str;

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
#[tokio::main]
async fn main() -> Result<()> {
    let config = load_config()?;

    if let Some(base_url) = &config.openai_base_url {
        std::env::set_var("OPENAI_BASE_URL", base_url);
    }
    if let Some(api_key) = &config.openai_api_key {
        std::env::set_var("OPENAI_API_KEY", api_key);
    }

    println!(
        "HTTP_PROXY: {}",
        std::env::var("http_proxy").unwrap_or_default()
    );
    println!(
        "OPENAI_BASE_URL: {}",
        std::env::var("OPENAI_BASE_URL").unwrap_or_default()
    );
    println!(
        "OPENAI_API_KEY: {}",
        std::env::var("OPENAI_API_KEY").unwrap_or_default()
    );

    run(&config).await
}

async fn run(config: &Config) -> Result<()> {
    let ignore_patterns = config
        .ignore
        .iter()
        .map(|s| Pattern::new(s))
        .collect::<Result<Vec<_>, _>>()?;

    let diffs = get_staged_diffs(Some(&ignore_patterns))?;

    if diffs.is_empty() {
        println!("No staged changes to commit after filtering.");
        return Ok(());
    }

    let model = &config.model;
    let language = config.language.as_deref();

    let client = Client::new();

    let (commit_msg, usage) = generate_commit_message(&client, &diffs, model, language).await?;
    // println!("\nGenerated commit message: {}", commit_msg);

    if let Some(usage) = usage {
        println!(
            "Tokens used: {}, input tokens: {}, output tokens: {}",
            usage.total_tokens, usage.prompt_tokens, usage.completion_tokens
        );
    }

    let (commit_hash, commit_message) = perform_commit(&commit_msg)?;
    println!("Commit {} {}", commit_hash, commit_message);

    Ok(())
}

fn get_staged_diffs(filter_patterns: Option<&Vec<Pattern>>) -> Result<String> {
    let output = Command::new("git")
        .args(["diff", "--staged", "--name-only"])
        .output()?;
    if !output.status.success() {
        return Err(anyhow!(
            "Failed to get staged files: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let staged_files = str::from_utf8(&output.stdout)?.lines();

    let filtered_files: Vec<&str> = staged_files
        .filter(|file| {
            if let Some(patterns) = filter_patterns {
                !patterns.iter().any(|p| p.matches(file))
            } else {
                true
            }
        })
        .collect();

    if filtered_files.is_empty() {
        return Ok(String::new());
    }

    let mut diff_command = Command::new("git");
    diff_command.arg("diff").arg("--staged").arg("--");
    diff_command.args(&filtered_files);

    let diff_output = diff_command.output()?;

    if !diff_output.status.success() {
        return Err(anyhow!(
            "Failed to get staged diffs: {}",
            String::from_utf8_lossy(&diff_output.stderr)
        ));
    }

    String::from_utf8(diff_output.stdout).map_err(|e| anyhow!(e))
}

async fn generate_commit_message(
    client: &Client<impl async_openai::config::Config>,
    diffs: &str,
    model: &str,
    language: Option<&str>,
) -> Result<(String, Option<CompletionUsage>)> {
    let user_content = format!("# Diffs:\n{}", diffs);

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

    println!("Waiting for LLM to generate commit message...");
    let response = client.chat().create(request).await?;

    if let Some(choice) = response.choices.into_iter().next() {
        let message = choice.message.content.unwrap_or_default();
        Ok((message.trim().trim_matches('"').to_string(), response.usage))
    } else {
        Err(anyhow!("Failed to get commit message from LLM"))
    }
}

fn perform_commit(message: &str) -> Result<(String, String)> {
    let output = Command::new("git")
        .args(["commit", "--no-verify", "-m", message])
        .output()?;

    if !output.status.success() {
        return Err(anyhow!(
            "Failed to commit: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let rev_parse_output = Command::new("git")
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
