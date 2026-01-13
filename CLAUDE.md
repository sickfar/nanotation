# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**nanotation** is a lightweight terminal-based annotation editor written in Rust. The binary is named `nanot` (not `nanotation`). It enables inline code review workflows using the `[ANNOTATION]` marker format with language-aware comment syntax.

**Version:** 0.1.1
**Language:** Rust (Edition 2024)
**Dependencies:** crossterm (terminal I/O), syntect (syntax highlighting), unicode-width

## Build & Development Commands

```bash
# Build the project
cargo build --verbose

# Run tests (limited coverage - only 2 unit tests in theme.rs)
cargo test --verbose

# Run the editor locally
cargo run -- <file>

# Install locally
cargo install --path .

# The installed binary is 'nanot'
nanot <file>
```

## Architecture Overview

### Core Design Patterns

1. **State Machine**: Mode-based editor with transitions:
   - `Normal` → `Annotating` → `Search` → `QuitPrompt` → `Help`
   - Defined in `models.rs::Mode` enum

2. **Event-Driven Architecture**:
   - Crossterm event loop in `editor.rs::run()`
   - Event processing in `event_handler.rs` (643 lines)
   - Debounced rendering in `ui.rs` (490 lines)

3. **Undo/Redo System**:
   - Action history tracking via `models.rs::Action` enum
   - Forward/backward traversal with `history` and `history_index` in `Editor` struct
   - Currently only supports `EditAnnotation` actions

4. **Lazy Syntax Highlighting**:
   - Per-line highlighting on render using syntect
   - Custom Zenbones (Alabaster) color scheme
   - Dark/Light theme toggling in `theme.rs`

### Module Responsibilities

| Module | Lines | Responsibility |
|--------|-------|----------------|
| `event_handler.rs` | 643 | Keyboard input processing; mode transitions; annotation editing |
| `ui.rs` | 490 | Terminal rendering; line numbers; gutter; annotation display |
| `editor.rs` | 380 | Editor state management; run loop; save/load; undo/redo |
| `text.rs` | 284 | Unicode-aware text wrapping with whitespace preservation |
| `file.rs` | 259 | File I/O; language detection; annotation parsing/serialization |
| `highlighting.rs` | 176 | Syntax highlighting integration with syntect |
| `theme.rs` | 71 | Dark/Light color schemes |
| `models.rs` | 22 | Core data structures: `Line`, `Mode`, `Action` |
| `main.rs` | 50 | CLI argument parsing; editor initialization |

### Key Data Structures

```rust
// models.rs
pub struct Line {
    pub content: String,
    pub annotation: Option<String>,
}

pub enum Mode {
    Normal,
    Annotating { buffer: String, cursor_pos: usize },
    Search { query: String, cursor_pos: usize },
    QuitPrompt,
    Help,
}

pub enum Action {
    EditAnnotation {
        line_index: usize,
        old_text: Option<String>,
        new_text: Option<String>,
    },
}

// editor.rs
pub struct Editor {
    pub lines: Vec<Line>,
    pub cursor_line: usize,
    pub scroll_offset: usize,
    pub mode: Mode,
    pub file_path: Option<String>,
    pub modified: bool,
    pub theme: Theme,
    pub lang_comment: String,      // Language-specific comment prefix
    pub search_matches: Vec<usize>,
    pub history: Vec<Action>,      // Undo/redo stack
    pub history_index: usize,
    pub highlighter: SyntaxHighlighter,
}
```

## Language Detection & Annotation Format

The editor auto-detects comment syntax based on file extension in `file.rs::detect_comment_style()`:

| Comment Style | Extensions |
|--------------|------------|
| `//` | .rs, .go, .java, .kt, .js, .ts, .c, .cpp, .h, .cs, .php, .scala, .dart, .swift |
| `#` | .py, .sh, .rb, .yaml, .yml, .toml, .pl, .r, Dockerfile |
| `--` | .sql, .lua, .hs, .ada |
| _(none)_ | .md |

**Annotation Format**: `<comment_prefix> [ANNOTATION] <text>`

Example:
- Rust: `// [ANNOTATION] Fix this logic`
- Python: `# [ANNOTATION] Add error handling`
- SQL: `-- [ANNOTATION] Optimize this query`
- Markdown: `[ANNOTATION] Clarify this section`

## AI Agent Integration

The `PROMPT.md` file contains formal instructions for AI agents to process `[ANNOTATION]` markers. Key workflow:

1. **Recursive Discovery**: Search for `[ANNOTATION]` in specified scope
2. **Context Analysis**: Evaluate 10+ lines around each marker
3. **Implementation**: Execute requested changes
4. **Verification**: Run syntax checks and tests
5. **Cleanup**: Remove `[ANNOTATION]` markers after verification

**Important**: Ignore `[ANNOTATION]` markers inside Markdown code blocks (triple backticks).

## Testing Notes

- **Limited Coverage**: Only 2 unit tests in `theme.rs` (Dark/Light theme color verification)
- No integration tests or end-to-end tests
- CI runs `cargo test --verbose` on push/PR to main
- When adding features, consider adding tests despite the current sparse coverage

## Common Development Patterns

### Adding a New Keyboard Shortcut

1. Add key handling in `event_handler.rs::handle_event()` or mode-specific handlers
2. Update help text in `ui.rs::render_help()`
3. If it modifies editor state, consider adding to `Action` enum for undo/redo

### Adding Support for a New Language

1. Add file extension mapping in `file.rs::detect_comment_style()`
2. Return appropriate comment prefix (`//`, `#`, `--`, or empty string)
3. Test annotation parsing/serialization with `file::parse_file()` and `file::serialize_file()`

### Modifying Theme Colors

1. Update `theme.rs::Theme::Dark` or `Theme::Light`
2. Colors are used in `ui.rs` for rendering different UI elements
3. Run existing tests: `cargo test theme_colors`

## Important Constraints

- **No External Config**: No config file support; settings are hardcoded
- **Single File Editing**: No multi-file support or file browser
- **Terminal Only**: No GUI; requires terminal with ANSI color support
- **History Granularity**: Undo/redo only tracks annotation edits, not cursor movements
