# nanotation

[![CI](https://github.com/sickfar/nanotation/actions/workflows/ci.yml/badge.svg)](https://github.com/sickfar/nanotation/actions/workflows/ci.yml)
[![Homebrew](https://img.shields.io/github/v/release/sickfar/nanotation?label=homebrew&logo=homebrew)](https://github.com/sickfar/homebrew-nanotation)

Small annotation editor for fast review process in Claude Code and similar console tools.
<img width="912" height="744" alt="image" src="https://github.com/user-attachments/assets/ba76acbc-e955-44f7-9dbb-6bd369485e32" />

## Quick Start

```bash
brew tap sickfar/nanotation
brew install nanotation
nanot <file>
```

## Why nanotation?

`nanotation` is a lightweight, terminal-based text editor specifically designed for code reviews and annotation workflows. Inspired by `nano` and `Antigravity`, it allows you to add inline review comments to any text file using the `[ANNOTATION]` marker format without breaking the file structure (comments are just comments).

**Key Features:**

-   **Inline Annotations**: Safely add review comments to any line.
-   **Git Diff View**: Side-by-side comparison with HEAD, word-level highlighting (`^D` to toggle).
-   **Keyboard Driven**: Efficient navigation (Arrow keys, `^N`/`^P` for jumping between annotations).
-   **AI Ready**: Designed to work seamlessly with AI Agents (Claude, etc.).
-   **Smart Change Detection**: Hash-based tracking - undo to original state = no unsaved changes.
-   **Zero Distraction**: Syntax highlighting, Theme toggling, and Unsaved changes protection.

## AI Agent Integration

`nanotation` allows you to communicate with AI agents directly through your codebase.

### 1. Setup Instructions
To enable your AI agent to understand and process your feedback:
1.  Copy the contents of **[PROMPT.md](./PROMPT.md)**.
2.  Add it to your agent's system instructions (e.g., `CLAUDE.md`, `AGENTS.md`, or Custom Instructions).

Once configured, you can ask your agent to "process all feedback" or "fix the code based on annotations," and it will implement changes and remove the markers automatically.

### 2. Claude Code Workflow
Use `nanotation` as your primary editor for reviewing plans in **Claude Code**.

**Configuration:**
Add this to your shell profile (`.bashrc` / `.zshrc`):
```bash
export EDITOR=nanot
```

**Workflow:**
1.  **Review Plans**: When Claude generates a plan, press `Ctrl+G` to open it in `nanotation`.
2.  **Annotate**: Scroll and press **Enter** on any line to add sticky notes/feedback.
3.  **Iterate**: Save and exit (`^O`, `^X`). Tell Claude: *"Fix the plan based on my annotations."*

## User Guide

### Controls

| Key | Action |
| :--- | :--- |
| `Ctrl` + `X` | Exit (prompts if unsaved) |
| `Ctrl` + `O` | Save File |
| `Enter` | **Add/Edit Annotation** |
| `Del` / `Backspace` | Delete Annotation |
| `Ctrl` + `N` / `P` | Next / Previous Annotation |
| `Ctrl` + `Z` / `Y` | Undo / Redo |
| `Ctrl` + `D` | Toggle Diff View (git) |
| `Ctrl` + `W` | Search |
| `Ctrl` + `T` | Toggle Theme |
| `Ctrl` + `G` | Show Help |
| `PgUp` / `PgDn` | Scroll Page |
| `Home` / `End` | Jump to Start / End |

### Diff View

When editing a file tracked by git, press `Ctrl+D` to toggle a side-by-side diff view:
- **Left pane**: HEAD version (last committed)
- **Right pane**: Working copy (current edits)
- **Word-level highlighting**: Changed words are highlighted within modified lines
- **Orange indicator**: Status bar shows `^D Diff` when diff is available

The diff view automatically strips annotations when comparing, so you see actual code changes.

### Annotation Format & Languages

Annotations are stored as native comments, ensuring the code remains compilable/runnable.

| Language / Type | Format Example | Strings/Prefix |
| :--- | :--- | :--- |
| **Rust, JS, C, Go, Java** | `// [ANNOTATION] ...` | `//` |
| **Python, Ruby, Shell, YAML** | `# [ANNOTATION] ...` | `#` |
| **SQL, Lua, Haskell** | `-- [ANNOTATION] ...` | `--` |
| **Markdown** | `[ANNOTATION] ...` | (None) |

## Installation Details

**From Source (Rust/Cargo):**

```bash
git clone https://github.com/sickfar/nanotation.git
cd nanotation
cargo install --path .
```
This installs the binary `nanot` to your Cargo bin directory.

## Contributing

Contributions are welcome! Please feel free to submit issues or pull requests.

## License

MIT
