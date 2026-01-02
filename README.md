# ai-commit-rs

`ai-commit-rs` is a Rust-based CLI tool that uses Large Language Models (LLMs) to automatically generate conventional git commit messages by analyzing your staged changes.

## 🚀 Features

- **Automated Messages**: Analyzes staged diffs and generates descriptive commit messages.
- **Conventional Commits**: Follows the `<type>: <description>` format (feat, fix, chore, etc.).
- **Highly Configurable**: Supports custom models, languages, and ignore patterns.
- **Privacy Focused**: Allows you to ignore specific files (like lock files or sensitive data) from being sent to the LLM.
- **Multi-Platform**: Works on Linux, macOS, and Windows.

## 📦 Installation

### From Source

Ensure you have [Rust and Cargo](https://rustup.rs/) installed:

```bash
git clone https://github.com/a690700752/ai-commit-rs.git
cd ai-commit-rs
cargo install --path .
```

### Pre-built Binaries

You can download pre-built binaries for your platform from the [Releases](https://github.com/a690700752/ai-commit-rs/releases) page.

## 🛠 Usage

1. **Stage your changes**:
   ```bash
   git add .
   ```

2. **Run `ai-commit-rs`**:
   ```bash
   ai-commit-rs
   ```

3. **Options**:
   - `--no-verify`: Skip git commit hooks.
   - `AI_COMMIT_DEBUG=1`: Enable debug logging to see what's being sent to the LLM.

## ⚙️ Configuration

Create a configuration file at `~/.ai-commit-rs.toml`.

### Example `~/.ai-commit-rs.toml`

```toml
# (Optional) OpenAI API base URL.
# Priority: config > env.OPENAI_BASE_URL > default
# openai_base_url = "https://api.openai.com/v1"

# (Optional) OpenAI API key.
# Priority: config > env.OPENAI_API_KEY
# openai_api_key = "sk-..."

# (Optional) The model to use. Defaults to "deepseek-v3-1".
model = "deepseek-v3-1"

# (Optional) The language for the commit message.
language = "English"

# (Optional) Glob patterns for files to ignore in the diff context.
ignore = [
    "*.lock",
    "target/",
    "node_modules/"
]
```

### Priority Order
1.  **Configuration File**: `~/.ai-commit-rs.toml`
2.  **Environment Variables**: `OPENAI_API_KEY`, `OPENAI_BASE_URL`
3.  **Defaults**: Internal defaults (DeepSeek model, English language, common ignore patterns).

## 📄 License

MIT or Apache-2.0 (standard Rust project licensing).
