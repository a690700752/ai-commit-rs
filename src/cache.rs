use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

const CACHE_FILE: &str = "ai-commit-rs-cache.json";

#[derive(Debug, Serialize, Deserialize)]
pub struct CommitCache {
    pub hash: String,
    pub message: String,
}

fn cache_path() -> PathBuf {
    PathBuf::from("/tmp").join(CACHE_FILE)
}

pub fn compute_diff_hash(diff: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(diff.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn load() -> Option<CommitCache> {
    let path = cache_path();
    let data = fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

pub fn save(diff_hash: &str, message: &str) -> Result<()> {
    let cache = CommitCache {
        hash: diff_hash.to_string(),
        message: message.to_string(),
    };
    let json = serde_json::to_string(&cache)?;
    fs::write(cache_path(), json)?;
    Ok(())
}


pub fn clear() {
    let _ = fs::remove_file(cache_path());
}
