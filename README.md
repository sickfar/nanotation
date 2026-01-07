# nanotation

[![CI](https://github.com/sickfar/nanotation/actions/workflows/ci.yml/badge.svg)](https://github.com/sickfar/nanotation/actions/workflows/ci.yml)


Small annotation editor for fast review process in Claude Code and similar console tools

## Overview

`nanotation` is a lightweight, terminal-based text editor specifically designed for code reviews and annotation workflows. Inspired by `nano` and `Antigravity`, it provides a simple interface for adding inline review comments to any text file using the `[ANNOTATION]` marker format, making it perfect for asynchronous code reviews in AI-assisted development environments.

## Features

-   **Inline Annotations**: Add comments to any line without modifying the original file content structure (annotations are saved inline but managed differently in UI).
-   **Keyboard Driven**: Efficient navigation and editing using standard and custom shortcuts.
-   **Search Functionality**: Jump to text matches within the file.
-   **Theme Toggling**: Switch between Light and Dark modes.
-   **Syntax Highlighting**: Basic language detection for comment styles.
-   **Line Numbers**: Clear context with line number gutter.
-   **Help Overlay**: In-editor keybinding reference (`^G`).
-   **Undo/Redo**: Safely revert annotation changes (`^Z`/`^Y`).
-   **Unsaved Changes Protection**: Warnings on exit if work is not saved.
-   **Quick Navigation**: Jump to next/previous annotation (`^N`/`^P`).

## Installation


1. **Clone the repository**:
   ```bash
   git clone https://github.com/sickfar/nanotation.git
   cd nanotation
   ```

2. **Install locally**:
   ```bash
   cargo install --path .
   ```
   This will install the binary `nanot` to your Cargo bin directory (usually `~/.cargo/bin`). Ensure this directory is in your `PATH`.

## Usage

```bash
nanot <file>
```

*Note: The file must exist.*

### Keyboard Shortcuts

| Key Combination | Action |
| :--- | :--- |
| `Ctrl` + `X` | Exit (prompts if unsaved changes) |
| `Ctrl` + `O` | Save File |
| `Ctrl` + `W` | Search |
| `Ctrl` + `T` | Toggle Theme |
| `Ctrl` + `G` | Toggle Help Overlay |
| `Ctrl` + `Z` | Undo Annotation Edit |
| `Ctrl` + `Y` | Redo Annotation Edit |
| `Ctrl` + `D` | Delete Annotation on current line |
| `Ctrl` + `N` | Jump to Next Annotation |
| `Ctrl` + `P` | Jump to Previous Annotation |
| `Enter` | Add/Edit Annotation on current line |
| `Up` / `Down` | Navigate Lines |
| `PgUp` / `PgDn` | Navigate Pages |
| `Alt` + `Up` / `Down` | Navigate Pages |
| `Esc` | Cancel annotation/search mode |


### Annotation Format

Annotations are stored as comments in the file format:

```rust
// [ANNOTATION] This function needs error handling
fn process_data(input: String) {
    // implementation
}
```

For Python files:
```python
# [ANNOTATION] Consider using a more descriptive variable name
x = 42
```

For Markdown files (no comment prefix):
```markdown
[ANNOTATION] Please fill the README file with more detailed information about the project
# nanotation
```

## Example Workflow

1. **Open a file for review**:
   ```bash
   nanot src/lib.rs
   ```

2. **Navigate to the line** you want to annotate using arrow keys

3. **Press Enter** to add an annotation

4. **Type your review comment** and press Enter to save

5. **Save the file** with `^O` when done

6. **Exit** with `^X`

The annotations will be preserved in the file and can be viewed/edited in subsequent sessions.

## Supported Languages

The editor automatically detects and uses appropriate comment styles:

- **C-style (`//`)**: Rust, Go, Java, Kotlin, JS/TS, C/C++, C#, Swift, PHP, Scala, Dart
- **Hash (`#`)**: Python, Ruby, Shell, YAML, TOML, Dockerfile, Perl, R
- **Double Dash (`--`)**: SQL, Lua, Haskell, Ada
- **Plain**: Markdown (no comment prefix)

## Contributing

Contributions are welcome! Please feel free to submit issues or pull requests.

## License

MIT
