use anyhow::{anyhow, Result};
use async_openai::types::chat::{
    ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
    CreateChatCompletionRequestArgs,
};
use async_openai::Client;
use git2::{Commit, ObjectType, Patch, Repository, Signature};
use regex::Regex;
use std::path::Path;

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

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    run().await
}

async fn run() -> Result<()> {
    let repo = Repository::open(".")?;
    let diffs = get_staged_diffs(&repo, Some(r".*\.lock$|.*\.log$|/target/"))?;

    if diffs.is_empty() {
        println!("No staged changes to commit after filtering.");
        return Ok(());
    }

    println!("# Diffs:\n{}", diffs);

    let commit_msg = generate_commit_message(&diffs).await?;
    println!("\nGenerated commit message: {}", commit_msg);

    let (commit_hash, commit_message) = perform_commit(&repo, &commit_msg)?;
    println!("Commit {} {}", commit_hash, commit_message);

    Ok(())
}

fn get_staged_diffs(repo: &Repository, filter_regex: Option<&str>) -> Result<String> {
    let head_tree = repo
        .head()
        .ok()
        .and_then(|head| head.peel_to_commit().ok())
        .and_then(|commit| commit.tree().ok());
    let diff = repo.diff_tree_to_index(head_tree.as_ref(), None, None)?;
    let regex = filter_regex.map(Regex::new).transpose()?;

    let mut diff_text = String::new();
    for i in 0..diff.deltas().len() {
        let delta = diff.deltas().nth(i).unwrap();
        if let Some(re) = &regex {
            let path = delta.new_file().path().unwrap_or(Path::new(""));
            if re.is_match(path.to_str().unwrap_or_default()) {
                continue;
            }
        }
        let patch = Patch::from_diff(&diff, i)?;
        if let Some(mut patch) = patch {
            let buf = patch.to_buf()?;
            diff_text.push_str(std::str::from_utf8(buf.as_ref()).unwrap_or(""));
        }
    }
    Ok(diff_text)
}

async fn generate_commit_message(diffs: &str) -> Result<String> {
    let client = Client::new();
    let user_content = format!("# Diffs:\n{}", diffs);

    let request = CreateChatCompletionRequestArgs::default()
        .model("gpt-3.5-turbo")
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

fn perform_commit(repo: &Repository, message: &str) -> Result<(String, String)> {
    let signature = Signature::now("ai-commit-rs", "ai-commit-rs@example.com")?;
    let mut index = repo.index()?;
    let tree_oid = index.write_tree()?;
    let tree = repo.find_tree(tree_oid)?;

    let parent_commit = find_head_commit(repo).ok();
    let parents: Vec<&Commit> = parent_commit.iter().collect();

    let commit_oid = repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        message,
        &tree,
        &parents,
    )?;

    let short_hash = repo
        .find_object(commit_oid, Some(ObjectType::Commit))?
        .short_id()?
        .as_str()
        .unwrap_or_default()
        .to_string();

    Ok((
        short_hash,
        message.lines().next().unwrap_or_default().to_string(),
    ))
}

fn find_head_commit(repo: &Repository) -> Result<Commit> {
    let head_ref = repo.head()?;
    head_ref
        .peel_to_commit()
        .map_err(|e| anyhow!("Failed to peel head to commit: {}", e))
}
