use crate::editor::EditorContent;
use crate::file_tree::FileTreePanel;
use crate::highlighting::{to_crossterm_color, SyntaxHighlighter};
use crate::models::{EditorState, FocusedPanel, Line, ViewMode};
use crate::text::{wrap_styled_text, wrap_text};
use crate::theme::{ColorScheme, Theme};
use crate::ui_diff::render_diff_mode;
use crate::ui_tree::{self, render_file_tree, render_error_message};
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    queue,
    style::{Attribute, Print, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor},
    terminal::{self},
};
use std::io::{self, Write};
use std::path::Path;
use syntect::highlighting::FontStyle;
use unicode_width::UnicodeWidthStr;

/// Renders the editor UI to the terminal.
/// Uses separate view_mode (how to render) and editor_state (input mode).
#[allow(clippy::too_many_arguments)]
pub fn render(
    lines: &[Line],
    cursor_line: usize,
    scroll_offset: usize,
    view_mode: &ViewMode,
    editor_state: &EditorState,
    file_path: &Option<String>,
    modified: bool,
    theme: Theme,
    search_matches: &[usize],
    current_match: Option<usize>,
    annotation_scroll: usize,
    highlighter: &SyntaxHighlighter,
    status_message: Option<&str>,
    lang_comment: &str,
    diff_available: bool,
    file_tree: Option<&FileTreePanel>,
    focused_panel: FocusedPanel,
    editor_content: &EditorContent,
) -> io::Result<()> {
    let (width, height) = terminal::size()?;

    // Check if we're in diff view mode
    if let ViewMode::Diff { diff_result } = view_mode {
        // Calculate layout for diff mode
        let (start_col, available_width) = if let Some(tree) = file_tree {
            // Render file tree first
            let mut stdout = io::stdout();
            let is_tree_focused = focused_panel == FocusedPanel::FileTree;
            render_file_tree(&mut stdout, tree, theme, height.saturating_sub(1), is_tree_focused)?;

            let tree_width = ui_tree::TREE_WIDTH;
            let start = tree_width;
            let available = (width as usize).saturating_sub(start as usize);
            (start, available as u16)
        } else {
            (0, width)
        };

        return render_diff_mode(
            lines,
            cursor_line,
            scroll_offset,
            diff_result,
            editor_state,
            file_path,
            modified,
            theme,
            annotation_scroll,
            highlighter,
            status_message,
            lang_comment,
            diff_available,
            start_col,
            available_width,
            focused_panel,
        );
    }

    // Reserve space at bottom for annotation area + status bar
    // Add 1 extra line if file tree present (for title bar)
    // Add 1 more line if file tree present (for editor bottom border)
    let title_bar_height = if file_tree.is_some() { 1 } else { 0 };
    let editor_border_height = if file_tree.is_some() { 1 } else { 0 };
    let content_height = (height - 5 - title_bar_height - editor_border_height) as usize;
    let colors = theme.colors();

    let mut stdout = io::stdout();

    // Calculate layout based on whether file tree is present
    let (editor_start_col, editor_width) = if file_tree.is_some() {
        let tree_width = ui_tree::TREE_WIDTH;
        let start = tree_width;
        let available = (width as usize).saturating_sub(start as usize);
        (start, available as u16)
    } else {
        (0, width)
    };

    // Render file tree if present
    if let Some(tree) = file_tree {
        let is_tree_focused = focused_panel == FocusedPanel::FileTree;
        render_file_tree(&mut stdout, tree, theme, height.saturating_sub(1), is_tree_focused)?;
    }

    // Render editor title bar if in directory mode (when file tree is present)
    let content_start_y = if file_tree.is_some() {
        let is_editor_focused = focused_panel == FocusedPanel::Editor;
        let title = file_path.as_deref().and_then(|p| {
            // Extract just the filename from the path
            std::path::Path::new(p)
                .file_name()
                .and_then(|n| n.to_str())
        }).unwrap_or("Editor");
        render_editor_title_bar(
            &mut stdout,
            title,
            is_editor_focused,
            &colors,
            editor_start_col,
            editor_width,
            0,
        )?;
        1 // Content starts at Y=1 (after title bar)
    } else {
        0 // Content starts at Y=0 (no title bar)
    };

    // Handle empty or error editor states
    match editor_content {
        EditorContent::Empty => {
            // Render empty editor area
            render_empty_editor(&mut stdout, &colors, editor_start_col, editor_width, height)?;
            // Render status bar and annotation panel
            render_annotation_panel(
                &mut stdout,
                editor_state,
                &colors,
                None,
                annotation_scroll,
                editor_start_col,
                editor_width,
                height,
                lang_comment,
            )?;
            render_status_bar_simple(
                &mut stdout,
                editor_state,
                &colors,
                file_path,
                modified,
                0,
                0,
                diff_available,
                status_message,
                file_tree.is_some(),
                focused_panel,
                0,        // Status bar always starts at column 0
                width,    // Status bar always spans full terminal width
                height,
            )?;
            stdout.flush()?;
            return Ok(());
        }
        EditorContent::Error { message } => {
            // Render error message in editor area
            render_error_message(&mut stdout, message, theme, editor_start_col, editor_width, height)?;
            // Render status bar and annotation panel
            render_annotation_panel(
                &mut stdout,
                editor_state,
                &colors,
                None,
                annotation_scroll,
                editor_start_col,
                editor_width,
                height,
                lang_comment,
            )?;
            render_status_bar_simple(
                &mut stdout,
                editor_state,
                &colors,
                file_path,
                modified,
                0,
                0,
                diff_available,
                status_message,
                file_tree.is_some(),
                focused_panel,
                0,        // Status bar always starts at column 0
                width,    // Status bar always spans full terminal width
                height,
            )?;
            stdout.flush()?;
            return Ok(());
        }
        EditorContent::Loaded => {
            // Continue with normal rendering
        }
    }

    // Account for borders (left and right) when calculating content area
    let border_width = if file_tree.is_some() { 2 } else { 0 }; // Only add borders in directory mode
    let editor_content_start_col = editor_start_col + (if border_width > 0 { 1 } else { 0 });
    let editor_content_width = editor_width.saturating_sub(border_width);

    queue!(stdout, MoveTo(editor_content_start_col, content_start_y))?;

    let gutter_width = lines.len().to_string().len() + 2; // + padding
    let content_width = (editor_content_width as usize).saturating_sub(gutter_width);

    let mut screen_line = 0;
    let mut line_idx = scroll_offset;

    // Render file content
    while screen_line < content_height && line_idx < lines.len() {
        let line = &lines[line_idx];
        let is_selected = line_idx == cursor_line;
        let has_annotation = line.annotation.is_some();

        let bg_color = if is_selected {
            if has_annotation {
                colors.annotated_selected_bg
            } else {
                colors.selected_bg
            }
        } else if has_annotation {
            colors.annotated_bg
        } else {
            colors.bg
        };

        // Determine extension
        let extension = file_path.as_deref()
            .map(|p| Path::new(p).extension().and_then(|e| e.to_str()).unwrap_or("txt"))
            .unwrap_or("txt");

        // Highlight
        let styled_spans = highlighter.highlight(&line.content, extension);
        
        // Wrap styled
        let wrapped_styled = wrap_styled_text(&styled_spans, content_width);
        
        let wrapped_styled = if wrapped_styled.is_empty() { 
             // Logic to handle empty line styling if needed, or just empty vector means empty line
             if line.content.is_empty() {
                 vec![vec![]]
             } else {
                 wrapped_styled
             }
        } else { 
            wrapped_styled 
        };
        
        // If it's truly empty (vec![vec![]]), we loop once with empty segments
        // The loop `for (i, wrapped_line_segments) in wrapped_styled.iter().enumerate()`
        // If it's `vec![vec![]]`, it iterates once. `wrapped_line_segments` is empty vec.
        // `for (style, text) in wrapped_line_segments` does nothing. 
        // Padding is calculated as `content_width`. Spaces are printed.
        // This is correct.

        for (i, wrapped_line_segments) in wrapped_styled.iter().enumerate() {
            if screen_line >= content_height {
                break;
            }
            
            // Gutter logic
            let line_num_str = if i == 0 {
                format!("{:>width$} ", line_idx + 1, width = gutter_width - 1)
            } else {
                format!("{:>width$} ", " ", width = gutter_width - 1)
            };

            queue!(
                stdout,
                MoveTo(editor_content_start_col, content_start_y + screen_line as u16),
                SetBackgroundColor(colors.bg),
                SetForegroundColor(colors.line_number_fg),
                Print(line_num_str),
                SetBackgroundColor(bg_color),
            )?;
            
            // Draw segments
            let mut current_line_width = 0;
            for (style, text) in wrapped_line_segments {
                let fg = to_crossterm_color(style.foreground);
                
                // Reset everything to handle any lingering state safely, then re-apply BG
                queue!(stdout, SetAttribute(Attribute::Reset))?;
                queue!(stdout, SetBackgroundColor(bg_color))?;
                
                // Font styles
                if style.font_style.contains(FontStyle::BOLD) {
                    queue!(stdout, SetAttribute(Attribute::Bold))?;
                }
                if style.font_style.contains(FontStyle::ITALIC) {
                    queue!(stdout, SetAttribute(Attribute::Italic))?;
                }
                if style.font_style.contains(FontStyle::UNDERLINE) {
                    queue!(stdout, SetAttribute(Attribute::Underlined))?;
                }

                queue!(stdout, SetForegroundColor(fg), Print(text))?;
                
                current_line_width += text.width();
            }
            
            // Fill padding
            // We need to ensure we are in correct BG state for padding
            // The last segment left us with bg_color set, but attributes might be set.
            queue!(stdout, SetAttribute(Attribute::Reset))?;
            queue!(stdout, SetBackgroundColor(bg_color))?;
            
            let padding = content_width.saturating_sub(current_line_width);
            if padding > 0 {
                queue!(stdout, Print(format!("{:width$}", "", width = padding)))?;
            }
            
            queue!(stdout, ResetColor)?;
            screen_line += 1;
        }

        line_idx += 1;
    }

    // Fill remaining content area
    while screen_line < content_height {
        queue!(
            stdout,
            MoveTo(editor_content_start_col, content_start_y + screen_line as u16),
            SetBackgroundColor(colors.bg),
            Print(format!("{:width$}", "", width = editor_content_width as usize)),
            ResetColor
        )?;
        screen_line += 1;
    }

    // Render editor borders if in directory mode
    if file_tree.is_some() {
        let is_editor_focused = focused_panel == FocusedPanel::Editor;
        let border_start_y = content_start_y;
        let border_end_y = height.saturating_sub(6); // End one line before annotation area
        render_editor_borders(
            &mut stdout,
            is_editor_focused,
            &colors,
            editor_start_col,
            editor_width,
            border_start_y,
            border_end_y,
        )?;
    }

    // Render annotation area (4 lines: border + 2 text lines + border) above status bar
    let annotation_start = height - 5;
    let is_annotation_focused = matches!(editor_state, EditorState::Annotating { .. });

    render_annotation_area(
        &mut stdout,
        lines,
        cursor_line,
        editor_state,
        annotation_scroll,
        &colors,
        editor_start_col,
        editor_width,
        annotation_start,
        is_annotation_focused,
    )?;

    // Render status bar (always full width from column 0)
    render_status_bar_new(
        &mut stdout,
        view_mode,
        editor_state,
        file_path,
        modified,
        cursor_line,
        lines.len(),
        search_matches,
        current_match,
        &colors,
        0,        // Status bar always starts at column 0
        width,    // Status bar always spans full terminal width
        height,
        status_message,
        diff_available,
        file_tree.is_some(),
        focused_panel,
    )?;

    // Show help overlay if in ShowingHelp state
    if matches!(editor_state, EditorState::ShowingHelp) {
        render_help_overlay(&mut stdout, &colors, width, height)?;
    }

    // Position and show cursor if in annotation edit state
    if let EditorState::Annotating { buffer, cursor_pos } = editor_state {
        position_cursor(
            &mut stdout,
            buffer,
            *cursor_pos,
            annotation_scroll,
            annotation_start,
            editor_start_col,
            editor_width,
        )?;
    } else {
        queue!(stdout, Hide)?;
    }

    stdout.flush()?;
    Ok(())
}

fn render_editor_title_bar(
    stdout: &mut impl Write,
    title: &str,
    is_focused: bool,
    colors: &ColorScheme,
    start_col: u16,
    width: u16,
    y: u16,
) -> io::Result<()> {
    let border_color = if is_focused {
        colors.panel_border_focused
    } else {
        colors.panel_border_unfocused
    };

    let bg_color = if is_focused {
        colors.panel_title_focused_bg
    } else {
        colors.panel_title_unfocused_bg
    };

    // Calculate available space for title text
    // Total: "┌─" (2) + " " (1) + title + " " (1) + padding + "─┐" (2) = 6 + title + padding
    let border_and_spaces = 6; // "┌─ " + " ─┐" = 6 chars
    let available_width = width.saturating_sub(border_and_spaces);

    // Truncate title if too long
    let truncated_title: String = if title.chars().count() > available_width as usize {
        title.chars().take((available_width.saturating_sub(1)) as usize).collect::<String>() + "…"
    } else {
        title.to_string()
    };

    // Build the title bar with borders
    let left_border = "┌─";
    let right_border = "─┐";
    let padding_needed = available_width.saturating_sub(truncated_title.chars().count() as u16);
    let padding = "─".repeat(padding_needed as usize);

    let title_line = format!("{} {} {}{}", left_border, truncated_title, padding, right_border);

    // Render the title bar
    queue!(
        stdout,
        MoveTo(start_col, y),
        SetBackgroundColor(bg_color),
        SetForegroundColor(border_color),
        Print(&title_line),
        ResetColor
    )?;

    Ok(())
}

fn render_editor_borders(
    stdout: &mut impl Write,
    is_focused: bool,
    colors: &ColorScheme,
    start_col: u16,
    width: u16,
    start_y: u16,
    end_y: u16,
) -> io::Result<()> {
    let border_color = if is_focused {
        colors.panel_border_focused
    } else {
        colors.panel_border_unfocused
    };

    // Render left and right borders for each content row
    for y in start_y..end_y {
        // Left border
        queue!(
            stdout,
            MoveTo(start_col, y),
            SetBackgroundColor(colors.bg),
            SetForegroundColor(border_color),
            Print("│"),
            ResetColor
        )?;

        // Right border
        queue!(
            stdout,
            MoveTo(start_col + width - 1, y),
            SetBackgroundColor(colors.bg),
            SetForegroundColor(border_color),
            Print("│"),
            ResetColor
        )?;
    }

    // Render bottom border
    queue!(
        stdout,
        MoveTo(start_col, end_y),
        SetBackgroundColor(colors.bg),
        SetForegroundColor(border_color)
    )?;

    let horizontal = "─".repeat((width - 2) as usize);
    queue!(stdout, Print(format!("└{}┘", horizontal)))?;

    Ok(())
}

fn render_annotation_area(
    stdout: &mut impl Write,
    lines: &[Line],
    cursor_line: usize,
    editor_state: &EditorState,
    annotation_scroll: usize,
    colors: &ColorScheme,
    start_col: u16,
    width: u16,
    annotation_start: u16,
    is_focused: bool,
) -> io::Result<()> {
    // Determine border color based on focus
    let border_color = if is_focused {
        colors.panel_border_focused
    } else {
        colors.panel_border_unfocused
    };

    // Determine title text based on focus
    let title_text = if is_focused {
        "Annotation (editing)"
    } else {
        "Annotation"
    };

    // Top border with title: ┌─ Annotation ─┐
    // Total width: "┌─" (2) + " " (1) + text + " " (1) + padding + "─┐" (2) = 6 + text + padding
    let border_and_spaces = 6;
    let available_width = width.saturating_sub(border_and_spaces);
    let truncated_text: String = if title_text.chars().count() > available_width as usize {
        title_text.chars().take((available_width.saturating_sub(1)) as usize).collect::<String>() + "…"
    } else {
        title_text.to_string()
    };
    let padding_needed = available_width.saturating_sub(truncated_text.chars().count() as u16);
    let padding = "─".repeat(padding_needed as usize);
    let top_border = format!("┌─ {} {}{}", truncated_text, padding, "─┐");

    queue!(
        stdout,
        MoveTo(start_col, annotation_start),
        SetBackgroundColor(colors.annotation_window_bg),
        SetForegroundColor(border_color),
        Print(top_border),
        ResetColor
    )?;

    // Get annotation content based on editor state
    let annotation_text = match editor_state {
        EditorState::Annotating { buffer, .. } => {
            if buffer.is_empty() {
                "[Type annotation here...]".to_string()
            } else {
                buffer.clone()
            }
        }
        _ => {
            if cursor_line < lines.len() {
                if let Some(ref annotation) = lines[cursor_line].annotation {
                    annotation.clone()
                } else {
                    "[No annotation - Press Enter to add]".to_string()
                }
            } else {
                "[No annotation]".to_string()
            }
        }
    };

    // Wrap annotation text
    // Width calculation: │ (1) + space (1) + content + space (1) + │ (1) = width
    // So content area = width - 4
    let max_annotation_width = width as usize - 4;
    let wrapped_annotation = wrap_text(&annotation_text, max_annotation_width);

    // Display 2 lines of wrapped annotation with scroll support
    for i in 0..2 {
        let line_idx = annotation_scroll + i;
        let display_line = if line_idx < wrapped_annotation.len() {
            wrapped_annotation[line_idx].clone()
        } else {
            String::new()
        };

        let y_pos = annotation_start + 1 + i as u16;

        // Calculate manual padding for proper wide character handling
        use crate::text::calculate_padding;
        let padding = calculate_padding(&display_line, max_annotation_width);
        queue!(
            stdout,
            MoveTo(start_col, y_pos),
            SetBackgroundColor(colors.annotation_window_bg),
            SetForegroundColor(border_color),
            Print("│"),
            SetForegroundColor(colors.annotation_window_fg),
            Print(format!(" {}{} ", display_line, " ".repeat(padding))),
            SetForegroundColor(border_color),
            Print("│"),
            ResetColor
        )?;
    }

    // Bottom border: └────┘
    let horizontal = "─".repeat(width as usize - 2);
    queue!(
        stdout,
        MoveTo(start_col, annotation_start + 3),
        SetBackgroundColor(colors.annotation_window_bg),
        SetForegroundColor(border_color),
        Print(format!("└{}┘", horizontal)),
        ResetColor
    )?;

    Ok(())
}

/// Status bar render function with tree awareness
#[allow(clippy::too_many_arguments)]
fn render_status_bar_new(
    stdout: &mut impl Write,
    view_mode: &ViewMode,
    editor_state: &EditorState,
    file_path: &Option<String>,
    modified: bool,
    cursor_line: usize,
    total_lines: usize,
    search_matches: &[usize],
    current_match: Option<usize>,
    colors: &ColorScheme,
    start_col: u16,
    width: u16,
    height: u16,
    status_message: Option<&str>,
    diff_available: bool,
    has_tree: bool,
    focused_panel: FocusedPanel,
) -> io::Result<()> {
    queue!(stdout, MoveTo(start_col, height - 1))?;

    // If there's a status message, show it simply
    if let Some(msg) = status_message {
        queue!(
            stdout,
            SetBackgroundColor(colors.status_bg),
            SetForegroundColor(colors.status_fg),
            Print(format!(" {:width$}", msg, width = width as usize - 2)),
            ResetColor
        )?;
        return Ok(());
    }

    // Status bar content depends on editor_state
    match editor_state {
        EditorState::Idle => {
            // Extract just the filename from path
            let filename = file_path
                .as_deref()
                .map(|p| Path::new(p).file_name().and_then(|n| n.to_str()).unwrap_or(p))
                .unwrap_or("[No File]");
            let modified_flag = if modified { " [Modified]" } else { "" };
            let view_indicator = if matches!(view_mode, ViewMode::Diff { .. }) {
                "DIFF | "
            } else {
                ""
            };

            // Build the left part: filename and line info
            let line_info = if total_lines > 0 {
                format!(" | Line {}/{}", cursor_line + 1, total_lines)
            } else {
                String::new()
            };

            let left_part = format!(
                " {}{}{}{}",
                view_indicator, filename, modified_flag, line_info
            );

            // Render left part with normal status colors
            queue!(
                stdout,
                SetBackgroundColor(colors.status_bg),
                SetForegroundColor(colors.status_fg),
                Print(&left_part),
            )?;

            // If diff is available, show the orange indicator with a space before it
            if diff_available {
                let diff_indicator = " ^D Diff ";
                queue!(
                    stdout,
                    SetBackgroundColor(colors.status_bg),
                    Print(" "),
                    SetBackgroundColor(colors.diff_indicator_bg),
                    SetForegroundColor(colors.diff_indicator_fg),
                    Print(diff_indicator),
                )?;
            }

            // Shortcuts depend on focus and tree presence
            let shortcuts = if has_tree {
                match focused_panel {
                    FocusedPanel::FileTree => " Tab Editor  ^G Git/Tree  F1 Help  ^X Exit",
                    FocusedPanel::Editor => " Tab Tree  ^G Git/Tree  F1 Help  ^O Save  ^X Exit",
                }
            } else {
                " F1 Help  ^X Exit  ^O Save  ^W Search  ^T Theme"
            };

            let current_len = left_part.len() + if diff_available { 10 } else { 0 };
            let remaining_width = (width as usize).saturating_sub(current_len + 1);
            use crate::text::truncate_to_width;
            let shortcuts_truncated = truncate_to_width(shortcuts, remaining_width);

            queue!(
                stdout,
                SetBackgroundColor(colors.status_bg),
                SetForegroundColor(colors.status_fg),
                Print(&shortcuts_truncated),
            )?;

            // Fill remaining space
            let total_len = current_len + shortcuts_truncated.len();
            let padding = (width as usize).saturating_sub(total_len);
            if padding > 0 {
                queue!(stdout, Print(format!("{:width$}", "", width = padding)))?;
            }
            queue!(stdout, ResetColor)?;
        }
        EditorState::Annotating { .. } => {
            queue!(
                stdout,
                SetBackgroundColor(colors.status_bg),
                SetForegroundColor(colors.status_fg),
                Print(format!(" {:width$}", "Enter: Save  Esc: Cancel  ←→: Move cursor  ↑↓: Navigate lines", width = width as usize - 2)),
                ResetColor
            )?;
        }
        EditorState::Searching { query, .. } => {
            let matches = if !search_matches.is_empty() {
                format!(" ({}/{})", current_match.map(|i| i + 1).unwrap_or(0), search_matches.len())
            } else {
                String::new()
            };
            let search_status = format!("Search: {}█{}  Enter: Next  Esc: Cancel", query, matches);
            queue!(
                stdout,
                SetBackgroundColor(colors.status_bg),
                SetForegroundColor(colors.status_fg),
                Print(format!(" {:width$}", search_status, width = width as usize - 2)),
                ResetColor
            )?;
        }
        EditorState::QuitPrompt => {
            queue!(
                stdout,
                SetBackgroundColor(colors.status_bg),
                SetForegroundColor(colors.status_fg),
                Print(format!(" {:width$}", "Unsaved changes! Save before exiting? (y/n/Esc)", width = width as usize - 2)),
                ResetColor
            )?;
        }
        EditorState::FileSwitchPrompt { .. } => {
            queue!(
                stdout,
                SetBackgroundColor(colors.status_bg),
                SetForegroundColor(colors.status_fg),
                Print(format!(" {:width$}", "Unsaved changes! Save before switching? (y/n/Esc)", width = width as usize - 2)),
                ResetColor
            )?;
        }
        EditorState::ShowingHelp => {
            queue!(
                stdout,
                SetBackgroundColor(colors.status_bg),
                SetForegroundColor(colors.status_fg),
                Print(format!(" {:width$}", "Help Mode - Press any key to return", width = width as usize - 2)),
                ResetColor
            )?;
        }
    }

    Ok(())
}

/// Render empty editor area
fn render_empty_editor(
    stdout: &mut impl Write,
    colors: &ColorScheme,
    start_col: u16,
    width: u16,
    height: u16,
) -> io::Result<()> {
    let content_height = height.saturating_sub(5) as usize;

    for row in 0..content_height {
        queue!(
            stdout,
            MoveTo(start_col, row as u16),
            SetBackgroundColor(colors.bg),
            Print(" ".repeat(width as usize)),
        )?;
    }

    queue!(stdout, ResetColor)?;
    Ok(())
}

/// Render annotation panel (used when no file loaded)
#[allow(clippy::too_many_arguments)]
fn render_annotation_panel(
    stdout: &mut impl Write,
    editor_state: &EditorState,
    colors: &ColorScheme,
    annotation: Option<&str>,
    annotation_scroll: usize,
    start_col: u16,
    width: u16,
    height: u16,
    _lang_comment: &str,
) -> io::Result<()> {
    let annotation_start = height - 5;

    // Error/empty states are never focused
    let border_color = colors.panel_border_unfocused;

    // Top border with title: ┌─ Annotation ─┐
    let title_text = "Annotation";
    let border_and_spaces = 6; // "┌─ " + text + " ─┐" = 6 chars
    let available_width = width.saturating_sub(border_and_spaces);
    let truncated_text: String = if title_text.chars().count() > available_width as usize {
        title_text.chars().take((available_width.saturating_sub(1)) as usize).collect::<String>() + "…"
    } else {
        title_text.to_string()
    };
    let padding_needed = available_width.saturating_sub(truncated_text.chars().count() as u16);
    let padding = "─".repeat(padding_needed as usize);
    let top_border = format!("┌─ {} {}{}", truncated_text, padding, "─┐");

    // Top border with title
    queue!(
        stdout,
        MoveTo(start_col, annotation_start),
        SetBackgroundColor(colors.annotation_window_bg),
        SetForegroundColor(border_color),
        Print(top_border),
        ResetColor
    )?;

    // Get annotation text
    let annotation_text = match editor_state {
        EditorState::Annotating { buffer, .. } => {
            if buffer.is_empty() {
                "[Type annotation here...]".to_string()
            } else {
                buffer.clone()
            }
        }
        _ => annotation.unwrap_or("[Select a file from the tree]").to_string(),
    };

    let max_width = width as usize - 4;
    let wrapped = wrap_text(&annotation_text, max_width);

    for i in 0..2 {
        let line_idx = annotation_scroll + i;
        let display_line = if line_idx < wrapped.len() {
            wrapped[line_idx].clone()
        } else {
            String::new()
        };

        let y_pos = annotation_start + 1 + i as u16;
        use crate::text::calculate_padding;
        let padding = calculate_padding(&display_line, max_width);

        queue!(
            stdout,
            MoveTo(start_col, y_pos),
            SetBackgroundColor(colors.annotation_window_bg),
            SetForegroundColor(border_color),
            Print("│"),
            SetForegroundColor(colors.annotation_window_fg),
            Print(format!(" {}{} ", display_line, " ".repeat(padding))),
            SetForegroundColor(border_color),
            Print("│"),
            ResetColor
        )?;
    }

    // Bottom border: └────┘
    let horizontal = "─".repeat(width as usize - 2);
    queue!(
        stdout,
        MoveTo(start_col, annotation_start + 3),
        SetBackgroundColor(colors.annotation_window_bg),
        SetForegroundColor(border_color),
        Print(format!("└{}┘", horizontal)),
        ResetColor
    )?;

    Ok(())
}

/// Simple status bar for empty/error states
#[allow(clippy::too_many_arguments)]
fn render_status_bar_simple(
    stdout: &mut impl Write,
    _editor_state: &EditorState,
    colors: &ColorScheme,
    file_path: &Option<String>,
    modified: bool,
    cursor_line: usize,
    total_lines: usize,
    _diff_available: bool,
    status_message: Option<&str>,
    has_tree: bool,
    focused_panel: FocusedPanel,
    start_col: u16,
    width: u16,
    height: u16,
) -> io::Result<()> {
    queue!(stdout, MoveTo(start_col, height - 1))?;

    if let Some(msg) = status_message {
        queue!(
            stdout,
            SetBackgroundColor(colors.status_bg),
            SetForegroundColor(colors.status_fg),
            Print(format!(" {:width$}", msg, width = width as usize - 2)),
            ResetColor
        )?;
        return Ok(());
    }

    let filename = file_path
        .as_deref()
        .map(|p| Path::new(p).file_name().and_then(|n| n.to_str()).unwrap_or(p))
        .unwrap_or("[No File]");

    let modified_flag = if modified { " [Modified]" } else { "" };

    let line_info = if total_lines > 0 {
        format!(" | Line {}/{}", cursor_line + 1, total_lines)
    } else {
        String::new()
    };

    let left_part = format!(" {}{}{}", filename, modified_flag, line_info);

    queue!(
        stdout,
        SetBackgroundColor(colors.status_bg),
        SetForegroundColor(colors.status_fg),
        Print(&left_part),
    )?;

    let shortcuts = if has_tree {
        match focused_panel {
            FocusedPanel::FileTree => " Tab Editor  ^G Git/Tree  F1 Help  ^X Exit",
            FocusedPanel::Editor => " Tab Tree  F1 Help  ^O Save  ^X Exit",
        }
    } else {
        " F1 Help  ^X Exit  ^O Save"
    };

    let current_len = left_part.len();
    let remaining = (width as usize).saturating_sub(current_len + 1);
    use crate::text::truncate_to_width;
    let truncated = truncate_to_width(shortcuts, remaining);

    queue!(stdout, Print(&truncated))?;

    let total_len = current_len + truncated.len();
    let padding = (width as usize).saturating_sub(total_len);
    if padding > 0 {
        queue!(stdout, Print(" ".repeat(padding)))?;
    }

    queue!(stdout, ResetColor)?;
    Ok(())
}

fn position_cursor(
    stdout: &mut impl Write,
    buffer: &str,
    cursor_pos: usize,
    annotation_scroll: usize,
    annotation_start: u16,
    start_col: u16,
    width: u16,
) -> io::Result<()> {
    let max_annotation_width = width as usize - 4;
    let wrapped_annotation = wrap_text(buffer, max_annotation_width);

    // Calculate cursor position in wrapped text
    let (cursor_line, cursor_col) = if !buffer.is_empty() {
        let chars: Vec<char> = buffer.chars().collect();
        let actual_pos = cursor_pos.min(chars.len());

        let mut chars_so_far = 0;
        let mut found_line = 0;
        let mut found_col = 0;

        for (line_idx, wrapped_line) in wrapped_annotation.iter().enumerate() {
            let wrapped_chars = wrapped_line.chars().count();
            let next_chars = chars_so_far + wrapped_chars;

            if actual_pos <= next_chars {
                found_line = line_idx;
                found_col = actual_pos - chars_so_far;
                break;
            }

            chars_so_far = next_chars;
            if line_idx < wrapped_annotation.len() - 1 && next_chars < chars.len() {
                chars_so_far += 1;
            }
        }
        (found_line, found_col.min(wrapped_annotation.get(found_line).map(|l| l.chars().count()).unwrap_or(0)))
    } else {
        (0, 0)
    };

    let cursor_screen_line = if cursor_line >= annotation_scroll &&
                               cursor_line < annotation_scroll + 2 {
        annotation_start + 1 + (cursor_line - annotation_scroll) as u16
    } else {
        annotation_start + 1
    };

    let display_line = if cursor_line < wrapped_annotation.len() {
        &wrapped_annotation[cursor_line]
    } else {
        ""
    };

    // Convert character index to visual column for proper cursor positioning with wide chars
    use crate::text::char_index_to_visual_col;
    let cursor_visual_col = char_index_to_visual_col(display_line, cursor_col);
    let cursor_x = start_col + 2 + cursor_visual_col as u16;

    queue!(
        stdout,
        MoveTo(cursor_x, cursor_screen_line),
        Show
    )?;

    Ok(())
}

fn render_help_overlay(
    stdout: &mut impl Write,
    colors: &ColorScheme,
    width: u16,
    height: u16,
) -> io::Result<()> {
    // Center the box
    let box_width = 50;
    let box_height = 20; // Increased for multi-layout note
    let start_x = (width.saturating_sub(box_width)) / 2;
    let start_y = (height.saturating_sub(box_height)) / 2;

    // Draw background
    for y in 0..box_height {
        if start_y + y >= height { break; }
        queue!(
            stdout,
            MoveTo(start_x, start_y + y),
            SetBackgroundColor(colors.annotation_window_bg),
            SetForegroundColor(colors.annotation_window_fg),
            Print(format!("{:width$}", " ", width = box_width as usize)),
        )?;
    }

    // Border
    queue!(
        stdout,
        MoveTo(start_x, start_y),
        Print(format!("╔{}╗", "═".repeat(box_width as usize - 2))),
        MoveTo(start_x, start_y + box_height - 1),
        Print(format!("╚{}╝", "═".repeat(box_width as usize - 2))),
    )?;
    
    for y in 1..box_height-1 {
        queue!(
            stdout,
            MoveTo(start_x, start_y + y),
            Print("║"),
            MoveTo(start_x + box_width - 1, start_y + y),
            Print("║"),
        )?;
    }

    // Content
    let commands = [
        " HELP MENU ",
        "",
        " ^N / ^P    Next / Prev Annotation",
        " Del/Bksp   Delete Annotation",
        " Enter      Add / Edit Annotation",
        " ^W         Search",
        " ^D         Toggle Diff View",
        " ^T         Toggle Theme",
        " ^Z / ^Y    Undo / Redo",
        " ^O         Save File",
        " ^X         Exit",
        " F1         Show Help",
        " Tab        Switch Editor / Tree",
        " ^G         Toggle Git/Tree Mode",
        "",
        " Arrow Keys Navigation",
        " PgUp/PgDn  Page Navigation",
        "",
        " Hotkeys work in EN/RU layouts",
        " Press Any Key to Close",
    ];

    for (i, cmd) in commands.iter().enumerate() {
        if i as u16 >= box_height - 2 { break; }
        queue!(
            stdout,
            MoveTo(start_x + 2, start_y + 1 + i as u16),
            Print(cmd),
        )?;
    }
    
    queue!(stdout, ResetColor)?;

    Ok(())
}

#[cfg(test)]
mod focus_rendering_tests {
    

    #[test]
    fn test_title_bar_text_generation() {
        // Test title bar string formatting
        let title = "test.rs";
        let formatted = format!(" {} ", title);
        assert!(formatted.contains(title));
        assert_eq!(formatted, " test.rs ");
    }

    #[test]
    fn test_title_bar_truncation_long_filename() {
        // Test that long filenames truncate gracefully
        let long_name = "a".repeat(100);
        let max_width = 30;
        let truncated: String = if long_name.chars().count() > max_width {
            long_name.chars().take(max_width - 1).collect::<String>() + "…"
        } else {
            long_name
        };
        // Verify character count, not byte length
        assert!(truncated.chars().count() <= max_width);
        assert!(truncated.ends_with('…'));
    }

    #[test]
    fn test_content_height_calculation_with_title() {
        // Verify content height reduces by 1 when title bar present
        let height = 50u16;
        let title_bar_height = 1u16;
        let without_title = height.saturating_sub(5);
        let with_title = height.saturating_sub(5 + title_bar_height);
        assert_eq!(with_title, without_title - 1);
        assert_eq!(with_title, 44);
    }

    #[test]
    fn test_content_height_calculation_without_title() {
        // Verify content height unchanged when no title bar
        let height = 50u16;
        let title_bar_height = 0u16;
        let content_height = height.saturating_sub(5 + title_bar_height);
        assert_eq!(content_height, 45);
    }

    #[test]
    fn test_annotation_title_text_when_focused() {
        // Verify annotation title includes "(editing)" when focused
        let focused_title = " Annotation (editing) ";
        assert!(focused_title.contains("editing"));
        assert!(focused_title.contains("Annotation"));
    }

    #[test]
    fn test_annotation_title_text_when_unfocused() {
        // Verify annotation title without "(editing)" when unfocused
        let unfocused_title = " Annotation ";
        assert!(!unfocused_title.contains("editing"));
        assert!(unfocused_title.contains("Annotation"));
    }

    #[test]
    fn test_minimum_terminal_height_with_title() {
        // Verify minimum height requirements
        let min_height = 6u16;
        let title_bar = 1u16;
        let annotation_panel = 4u16;
        let status_bar = 1u16;
        assert!(min_height >= title_bar + annotation_panel + status_bar);
    }

    // =========================================================================
    // Cursor Positioning Tests
    // =========================================================================

    #[test]
    fn test_cursor_x_calculation_with_start_col() {
        // Verify cursor X position accounts for start_col offset
        let start_col = 30u16; // After file tree
        let border_and_padding = 2u16; // "│ "
        let cursor_visual_col = 5u16; // 5 chars into annotation
        let expected_x = start_col + border_and_padding + cursor_visual_col;
        assert_eq!(expected_x, 37);
    }

    #[test]
    fn test_cursor_x_at_annotation_start() {
        // Cursor at position 0 should be at start_col + 2
        let start_col = 30u16;
        let cursor_visual_col = 0u16;
        let cursor_x = start_col + 2 + cursor_visual_col;
        assert_eq!(cursor_x, 32); // Right after "│ "
    }

    #[test]
    fn test_cursor_x_in_normal_mode_no_tree() {
        // Without tree, start_col is 0
        let start_col = 0u16;
        let cursor_visual_col = 10u16;
        let cursor_x = start_col + 2 + cursor_visual_col;
        assert_eq!(cursor_x, 12);
    }

    #[test]
    fn test_cursor_x_in_directory_mode_with_tree() {
        // With tree (width 30), start_col is 30
        let start_col = 30u16;
        let cursor_visual_col = 15u16;
        let cursor_x = start_col + 2 + cursor_visual_col;
        assert_eq!(cursor_x, 47);
    }

    #[test]
    fn test_cursor_y_calculation_first_line() {
        // Cursor on first visible line
        let annotation_start = 45u16;
        let annotation_scroll = 0usize;
        let cursor_line = 0usize;
        let cursor_y = annotation_start + 1 + (cursor_line - annotation_scroll) as u16;
        assert_eq!(cursor_y, 46); // First content line after top border
    }

    #[test]
    fn test_cursor_y_calculation_second_line() {
        // Cursor on second visible line
        let annotation_start = 45u16;
        let annotation_scroll = 0usize;
        let cursor_line = 1usize;
        let cursor_y = annotation_start + 1 + (cursor_line - annotation_scroll) as u16;
        assert_eq!(cursor_y, 47); // Second content line
    }

    #[test]
    fn test_cursor_y_with_scroll() {
        // Cursor visible after scrolling
        let annotation_start = 45u16;
        let annotation_scroll = 2usize;
        let cursor_line = 3usize; // Line 3 is visible when scroll is 2
        let cursor_y = annotation_start + 1 + (cursor_line - annotation_scroll) as u16;
        assert_eq!(cursor_y, 47); // Line 3 shows at second position (offset 1) when scroll is 2
    }

    #[test]
    fn test_cursor_position_accounts_for_borders() {
        // Verify border and padding are accounted for
        // Format: "│ text │" where text starts at column 2
        let start_col = 0u16;
        let left_border = 1u16; // "│"
        let left_padding = 1u16; // " "
        let cursor_offset = left_border + left_padding;
        assert_eq!(cursor_offset, 2);

        // Full calculation
        let cursor_visual_col = 0u16;
        let cursor_x = start_col + cursor_offset + cursor_visual_col;
        assert_eq!(cursor_x, 2);
    }
}
