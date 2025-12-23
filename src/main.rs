use anyhow::{anyhow, Result};
use async_openai::types::chat::{
    ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
    CreateChatCompletionRequestArgs,
};
use async_openai::Client;
use glob::Pattern;
use std::fs;
use std::process::Command;
use std::str;

const SYSTEM_PROMPT: &str = r#"You are an expert software engineer that generates concise, one-line Git commit messages based on the provided diffs.
Review the provided context and diffs which are about to be committed to a git repo.
Review the diffs carefully.
Generate a one-line commit message for those changes.
The commit message should be structured as follows: <type>: <description>
Use these for <type>: fix, feat, build, chore, ci, docs, style, refactor, perf, test

Ensure the commit message:
- Is in the imperative mood (e.g., "add feature" not "added feature" or "adding feature").
- Does not exceed 72 characters.

Reply only with the one-line commit message, without any additional text, explanations, or line breaks."#;

const IGNORE_FILE_NAME: &str = ".ai-commit-ignore";

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv_override().ok();
    // print http_proxy env
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
    run().await
}

async fn run() -> Result<()> {
    let ignore_patterns = read_ignore_patterns(IGNORE_FILE_NAME).unwrap_or_else(|_| Vec::new());
    let diffs = get_staged_diffs(Some(&ignore_patterns))?;

    if diffs.is_empty() {
        println!("No staged changes to commit after filtering.");
        return Ok(());
    }

    // println!("# Diffs:\n{}", diffs);

    let commit_msg = generate_commit_message(&diffs).await?;
    println!("\nGenerated commit message: {}", commit_msg);

    let (commit_hash, commit_message) = perform_commit(&commit_msg)?;
    println!("Commit {} {}", commit_hash, commit_message);

    Ok(())
}

fn read_ignore_patterns(file_path: &str) -> Result<Vec<Pattern>> {
    let content = fs::read_to_string(file_path)?;
    content
        .lines()
        .map(|line| Pattern::new(line).map_err(|e| anyhow!(e)))
        .collect()
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

async fn generate_commit_message(diffs: &str) -> Result<String> {
    let client = Client::new();
    let user_content = format!("# Diffs:\n{}", diffs);

    let request = CreateChatCompletionRequestArgs::default()
        .model("deepseek-v3-1")
        .messages([
            ChatCompletionRequestSystemMessageArgs::default()
                .content(SYSTEM_PROMPT)
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
        Ok(message.trim().trim_matches('"').to_string())
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
