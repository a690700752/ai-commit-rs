# Aider Git Commit Logic Documentation

This document provides a detailed explanation of a git commit function. The goal is to offer a comprehensive guide for understanding and reimplementing this functionality in any programming language.

## 1. Overview

The `commit` function is responsible for committing file changes from the Git staging area (index). Its key feature is the ability to automatically generate a commit message using a Language Model (LLM) by analyzing the code differences (diffs) of the staged files.

## 2. Core Workflow

The function follows these main steps:

1.  **Check for Staged Changes**: It first checks if there are any files in the staging area. If the index is empty, the function exits.
2.  **Gather Diffs**: It collects the diffs of the staged files. During this process, it filters out files matching a configurable regular expression (e.g., lock files) so they are not sent to the LLM. If there are no diffs remaining after filtering, it exits.
3.  **Generate Commit Message**: It generates a commit message using an LLM (see [Commit Message Generation](#3-commit_message_generation)).
4.  **Construct and Execute `git commit`**: It builds and executes the final `git commit` command, which commits all files currently in the staging area.
5.  **Return Value**: It returns the commit hash and message upon success, or `None` if an error occurs or there's nothing to commit.

---

## 3. Commit Message Generation

The `commit` function calls `get_commit_message()` to generate the commit message.

### 3.1. `get_commit_message(diffs, context, user_language=None)`

1.  **Prepare Content for LLM**:
    *   It prepends the collected `diffs` with a header `# Diffs:
`.
    *   If any additional `context` string is provided, it is added before the diffs. This `content` is the main user-provided input for the LLM.

2.  **Prepare System Prompt**:
    *   It uses a system prompt, which is detailed in the next section.
    *   If a `user_language` is specified (e.g., "Chinese"), it appends an instruction to the system prompt, like `
- Is written in Chinese.`. The `{language_instruction}` placeholder in the prompt is replaced with this text.

3.  **Invoke LLM**:
    *   It uses a single, pre-configured LLM `model`.
    *   It constructs the message payload, consisting of the system prompt and the user content (context + diffs).
    *   If the total number of tokens exceeds the model's maximum input limit, the process may fail.
    *   It sends the request to the model. A waiting spinner is shown during this process.

4.  **Process and Return Message**:
    *   If the model fails to generate a message, an error is reported, and the function returns `None`.
    *   The received message is stripped of leading/trailing whitespace and any surrounding quotation marks.
    *   The cleaned message is returned.

### 3.2. System Prompt for Commit Message Generation

The following template is used as the system prompt for the LLM.

```
You are an expert software engineer that generates concise, one-line Git commit messages based on the provided diffs.
Review the provided context and diffs which are about to be committed to a git repo.
Review the diffs carefully.
Generate a one-line commit message for those changes.
The commit message should be structured as follows: <type>: <description>
Use these for <type>: fix, feat, build, chore, ci, docs, style, refactor, perf, test

Ensure the commit message:{language_instruction}
- Starts with the appropriate prefix.
- Is in the imperative mood (e.g., "add feature" not "added feature" or "adding feature").
- Does not exceed 72 characters.

Reply only with the one-line commit message, without any additional text, explanations, or line breaks.
```

---

## 4. Executing the Commit

1.  **Prepare Commit Command (`cmd`)**:
    *   The final commit message is added with the `-m` flag.
    *   If `git_commit_verify` is `False`, the `--no-verify` flag is appended.
    *   The command will commit whatever is currently in the staging area. No file list is passed to the command.

2.  **Invoke `git.commit()`**:
    *   The `self.repo.git.commit(cmd)` method is called, which executes the `git commit -m "..."` command in a subprocess.

3.  **Handle Outcome**:
    *   On success, it retrieves the new commit's hash, prints a confirmation message to the user (`Commit <hash> <message>`), and returns the hash and message.
    *   If any `git.exc.GitError` (or other related exceptions) occurs, it reports the error to the user and returns `None`.

## 5. Helper Functions and Dependencies

*   **`get_diffs()`**: This function is responsible for generating the diffs from the staged files (`git diff --cached`).
    *   **Filtering**: Before returning the diffs, it filters out changes from files whose names match a configurable regular expression (e.g., `.*\.lock$`). This prevents noisy diffs from being sent to the LLM. Note that the files themselves are still part of the commit.
*   **Dependencies**: The primary dependency is the `GitPython` library (`git`), which provides the interface to the Git repository. An LLM integration (via a `model` object) is required for automatic message generation.
