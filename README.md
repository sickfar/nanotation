# nanotation

Small annotation editor for fast review process in Claude Code and similar console tools

## Overview

`nanotation` is a lightweight, terminal-based text editor specifically designed for code reviews and annotation workflows. Inspired by `nano`, it provides a simple interface for adding inline review comments to any text file using the `[ANNOTATION]` marker format, making it perfect for asynchronous code reviews in AI-assisted development environments.

## Features

- **Inline Annotations**: Add review comments directly above code lines using the `[ANNOTATION]` marker
- **Dedicated Annotation Area**: A dedicated 4-line display area above the status bar with 2 lines for text inside ASCII borders. Features a visible cursor and normal text editor navigation (←→ to move cursor, ↑↓ to navigate through wrapped lines) with automatic scrolling
- **Multi-Language Support**: Automatically detects comment styles for various programming languages (Rust, Go, Java, Kotlin, JavaScript, Python, etc.)
- **Visual Highlighting**: Annotated lines are highlighted with distinct colors for easy identification
- **Theme Support**: Toggle between dark and light themes (`^T`)
- **Search Functionality**: Full-text search with match navigation (`^W`)
- **Text Wrapping**: Automatic line wrapping with indentation preservation
- **Keyboard-Driven**: Efficient keyboard shortcuts for fast navigation and editing


## Installation

### From Source

```bash
git clone https://github.com/yourusername/nanotation.git
cd nanotation
cargo build --release
```

The binary will be available at `target/release/nanotation`.

### Install Globally

```bash
cargo install --path .
```

## Usage

### Basic Usage

```bash
# Open an existing file
nanotation src/main.rs

# View help
nanotation --help
```

### Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `^X` | Exit the editor |
| `^O` | Save file |
| `^W` | Search mode |
| `^T` | Toggle theme (dark/light) |
| `^D` | Delete annotation on current line |
| `Enter` | Add/edit annotation for current line |
| `↑` / `↓` | Navigate up/down (when editing annotation: move cursor through wrapped lines) |
| `←` / `→` | When editing annotation: move cursor left/right |
| `PgUp` / `PgDn` | Page navigation |
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
   nanotation src/lib.rs
   ```

2. **Navigate to the line** you want to annotate using arrow keys

3. **Press Enter** to add an annotation

4. **Type your review comment** and press Enter to save

5. **Save the file** with `^O` when done

6. **Exit** with `^X`

The annotations will be preserved in the file and can be viewed/edited in subsequent sessions.

## Supported Languages

The editor automatically detects and uses appropriate comment styles:

- **C-style (`//`)**: Rust, Go, Java, Kotlin, JavaScript, TypeScript, C, C++
- **Hash (`#`)**: Python, Shell, Ruby
- **Plain**: Markdown (no comment prefix)

## Project Structure

The codebase is organized into focused modules for maintainability:

```
src/
├── main.rs          - Entry point and CLI
├── theme.rs         - Theme and color schemes
├── models.rs        - Core data structures (Line, Mode)
├── text.rs          - Text wrapping utilities
├── file.rs          - File I/O and parsing
├── ui.rs            - Terminal rendering
├── event_handler.rs - Keyboard event handling
└── editor.rs        - Main editor coordination
```

The project includes **18 unit tests** covering critical functionality like text wrapping, file parsing, comment detection, and search operations.

## Contributing

Contributions are welcome! Please feel free to submit issues or pull requests.

## License

MIT
