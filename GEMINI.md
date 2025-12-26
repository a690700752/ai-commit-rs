# ai-commit-rs

## Project Overview

`ai-commit-rs` is a Rust-based CLI tool designed to automatically generate conventional git commit messages using Large Language Models (LLMs). It analyzes the staged changes in your git repository and leverages an OpenAI-compatible API to craft concise, descriptive commit messages.

## Key Features

*   **Automated Commit Messages**: Generates commit messages based on staged diffs.
*   **Conventional Commits**: Follows the conventional commit format (e.g., `feat: ...`, `fix: ...`).
*   **Configurable**: Customizable via a TOML configuration file or environment variables.
*   **Ignore Patterns**: Supports ignoring specific files (like lock files) from the context sent to the LLM, while still including them in the commit.
*   **Multi-Language Support**: Can generate commit messages in different languages.
*   **Model Selection**: Allows choosing the specific LLM model to use (default: `deepseek-v3-1`).

## Architecture & Logic

*   **Language**: Rust
*   **Entry Point**: `src/main.rs` handles the CLI arguments, configuration loading, and the main execution flow.
*   **Core Logic**:
    1.  **Diff Collection**: Retrieves staged changes using `git diff --staged`.
    2.  **Filtering**: Filters out files based on configured ignore patterns.
    3.  **LLM Interaction**: Sends the diffs to the configured LLM API using `async-openai`.
    4.  **Commit Execution**: Executes `git commit` with the generated message.
*   **Dependencies**:
    *   `tokio`: Asynchronous runtime.
    *   `async-openai`: OpenAI API client.
    *   `clap`: Command-line argument parsing.
    *   `anyhow` / `thiserror`: Error handling.
    *   `dirs`: Platform-independent home directory resolution.

## Configuration

Configuration is loaded from `~/.ai-commit-rs.toml`. An example configuration is available in `example.toml`.

### Priority Order
1.  **Config File**: Settings in `~/.ai-commit-rs.toml`.
2.  **Environment Variables**: `OPENAI_BASE_URL`, `OPENAI_API_KEY`.
3.  **Defaults**: Internal default values.

### Key Configuration Options
*   `openai_base_url`: Base URL for the LLM API (default: `https://api.openai.com/v1`).
*   `openai_api_key`: API key for authentication.
*   `model`: The model identifier to use (default: `deepseek-v3-1`).
*   `language`: Target language for the commit message (e.g., "English", "Chinese").
*   `ignore`: List of glob patterns to exclude from analysis (e.g., `["*.lock", "target/"]`).

## Building and Running

### Prerequisites
*   Rust (latest stable toolchain)
*   Git

### Build
```bash
cargo build --release
```

### Run
```bash
cargo run -- [OPTIONS]
```

### Installation
To install the binary locally:
```bash
cargo install --path .
```

## Development Conventions

*   **Code Style**: Follow standard Rust formatting (`cargo fmt`) and linting (`cargo clippy`).
*   **Error Handling**: Use `anyhow::Result` for application-level errors.
*   **Async/Await**: The application is asynchronous, utilizing `tokio` for I/O-bound operations (network requests, git commands).
*   **Documentation**: Refer to `docs/commit_logic.md` for the underlying logic of commit message generation.
