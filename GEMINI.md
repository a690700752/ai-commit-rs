# GEMINI 初始化

这是一个使用 Rust 开发的 AI Commit 工具, 它将根据 `docs/commit_logic.md` 的逻辑进行实现.

主要技术栈:
- **Rust**: 核心开发语言
- **Tokio**: 用于异步处理
- **async-openai**: 与 LLM (OpenAI) 进行通信
- **git2-rs**: 用于与 Git 仓库进行交互
