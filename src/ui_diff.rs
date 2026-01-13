//! Diff mode UI rendering module.

use crate::diff::{ChangeType, DiffResult, LineChange, WordChange};
use crate::highlighting::{to_crossterm_color, SyntaxHighlighter};
use crate::models::{EditorState, Line};
use crate::text::wrap_text;
use crate::theme::{ColorScheme, Theme};
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    queue,
    style::{Attribute, Color, Print, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor},
    terminal,
};
use std::io::{self, Write};
use std::path::Path;
use unicode_width::UnicodeWidthStr;

/// Renders the editor in diff mode with split panes.
/// Now accepts EditorState to properly handle annotation editing, help, etc.
#[allow(clippy::too_many_arguments)]
pub fn render_diff_mode(
    lines: &[Line],
    cursor_line: usize,
    scroll_offset: usize,
    diff_result: &DiffResult,
    editor_state: &EditorState,
    file_path: &Option<String>,
    modified: bool,
    theme: Theme,
    annotation_scroll: usize,
    highlighter: &SyntaxHighlighter,
    status_message: Option<&str>,
    _lang_comment: &str,
    diff_available: bool,
) -> io::Result<()> {
    let (width, height) = terminal::size()?;
    let content_height = (height.saturating_sub(5)) as usize;
    let colors = theme.colors();

    let mut stdout = io::stdout();
    queue!(stdout, MoveTo(0, 0))?;

    // Calculate pane widths
    let separator_width = 1;
    let total_content_width = width as usize - separator_width;
    let left_pane_width = total_content_width / 2;
    let right_pane_width = total_content_width - left_pane_width;

    // Calculate gutter widths for each pane
    let left_gutter_width = lines.len().to_string().len() + 2;
    let right_gutter_width = diff_result
        .lines
        .iter()
        .filter_map(|dl| dl.head.as_ref().map(|(n, _, _)| *n))
        .max()
        .unwrap_or(1)
        .to_string()
        .len()
        + 2;

    let left_content_width = left_pane_width.saturating_sub(left_gutter_width);
    let right_content_width = right_pane_width.saturating_sub(right_gutter_width);

    // Determine extension for syntax highlighting
    let extension = file_path
        .as_deref()
        .map(|p| {
            Path::new(p)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("txt")
        })
        .unwrap_or("txt");

    let mut screen_line = 0;
    let mut diff_line_idx = scroll_offset;

    // Render diff content
    while screen_line < content_height && diff_line_idx < diff_result.lines.len() {
        let diff_line = &diff_result.lines[diff_line_idx];

        // Determine if this diff line corresponds to the cursor
        let is_cursor_line = diff_line
            .working
            .as_ref()
            .map(|(n, _, _)| *n == cursor_line + 1)
            .unwrap_or(false);

        // Render left pane (working copy)
        render_diff_pane_line(
            &mut stdout,
            &diff_line.working,
            0,
            left_gutter_width,
            left_content_width,
            is_cursor_line,
            true, // is_left_pane
            &colors,
            highlighter,
            extension,
            screen_line as u16,
            lines,
        )?;

        // Render separator
        queue!(
            stdout,
            MoveTo(left_pane_width as u16, screen_line as u16),
            SetBackgroundColor(colors.status_bg),
            SetForegroundColor(colors.status_fg),
            Print("│"),
            ResetColor
        )?;

        // Render right pane (HEAD)
        render_diff_pane_line(
            &mut stdout,
            &diff_line.head,
            (left_pane_width + separator_width) as u16,
            right_gutter_width,
            right_content_width,
            false, // cursor is only on left
            false, // is_left_pane
            &colors,
            highlighter,
            extension,
            screen_line as u16,
            lines,
        )?;

        screen_line += 1;
        diff_line_idx += 1;
    }

    // Fill remaining content area
    while screen_line < content_height {
        // Left pane empty
        queue!(
            stdout,
            MoveTo(0, screen_line as u16),
            SetBackgroundColor(colors.bg),
            Print(format!("{:width$}", "", width = left_pane_width)),
        )?;

        // Separator
        queue!(
            stdout,
            MoveTo(left_pane_width as u16, screen_line as u16),
            SetBackgroundColor(colors.status_bg),
            SetForegroundColor(colors.status_fg),
            Print("│"),
        )?;

        // Right pane empty
        queue!(
            stdout,
            MoveTo((left_pane_width + separator_width) as u16, screen_line as u16),
            SetBackgroundColor(colors.bg),
            Print(format!("{:width$}", "", width = right_pane_width)),
            ResetColor
        )?;

        screen_line += 1;
    }

    // Render annotation area
    let annotation_start = height - 5;
    render_diff_annotation_area(
        &mut stdout,
        lines,
        cursor_line,
        editor_state,
        annotation_scroll,
        &colors,
        width,
        annotation_start,
    )?;

    // Render status bar
    render_diff_status_bar(
        &mut stdout,
        editor_state,
        file_path,
        modified,
        cursor_line,
        lines.len(),
        &colors,
        width,
        height,
        status_message,
        diff_available,
    )?;

    // Render help overlay if showing help
    if matches!(editor_state, EditorState::ShowingHelp) {
        render_diff_help_overlay(&mut stdout, &colors, width, height)?;
    }

    // Position and show cursor if in annotation edit state
    if let EditorState::Annotating { buffer, cursor_pos } = editor_state {
        position_diff_cursor(
            &mut stdout,
            buffer,
            *cursor_pos,
            annotation_scroll,
            annotation_start,
            width,
        )?;
    } else {
        queue!(stdout, Hide)?;
    }

    stdout.flush()?;
    Ok(())
}

/// Renders a single line in a diff pane.
#[allow(clippy::too_many_arguments)]
fn render_diff_pane_line(
    stdout: &mut impl Write,
    line_data: &Option<(usize, String, LineChange)>,
    start_x: u16,
    gutter_width: usize,
    content_width: usize,
    is_cursor_line: bool,
    is_left_pane: bool,
    colors: &ColorScheme,
    highlighter: &SyntaxHighlighter,
    extension: &str,
    y: u16,
    lines: &[Line],
) -> io::Result<()> {
    queue!(stdout, MoveTo(start_x, y))?;

    match line_data {
        Some((line_num, content, change)) => {
            // Determine background color based on change type and cursor
            let (line_bg, word_added_bg, word_removed_bg) = get_diff_colors(change, is_cursor_line, is_left_pane, colors);

            // Render gutter
            let line_num_str = format!("{:>width$} ", line_num, width = gutter_width - 1);
            queue!(
                stdout,
                SetBackgroundColor(colors.bg),
                SetForegroundColor(colors.line_number_fg),
                Print(&line_num_str),
            )?;

            // Check if this line has an annotation (only show on left pane)
            let has_annotation = if is_left_pane && *line_num > 0 && *line_num <= lines.len() {
                lines[*line_num - 1].annotation.is_some()
            } else {
                false
            };

            // Adjust background for annotated lines
            let line_bg = if has_annotation && is_cursor_line {
                colors.annotated_selected_bg
            } else if has_annotation {
                colors.annotated_bg
            } else {
                line_bg
            };

            queue!(stdout, SetBackgroundColor(line_bg))?;

            // Render content with word-level highlighting if modified
            match change {
                LineChange::Modified { words, old_leading_ws, new_leading_ws } => {
                    let leading_ws = if is_left_pane { new_leading_ws } else { old_leading_ws };
                    render_word_diff(stdout, words, leading_ws, line_bg, word_added_bg, word_removed_bg, is_left_pane, content_width)?;
                }
                _ => {
                    // Simple highlight for added/removed/unchanged lines
                    let styled_spans = highlighter.highlight(content, extension);
                    let mut current_width = 0;
                    for (style, text) in styled_spans {
                        if current_width >= content_width {
                            break;
                        }
                        let fg = to_crossterm_color(style.foreground);
                        // Use truncate_to_width for proper wide character handling
                        use crate::text::truncate_to_width;
                        let remaining_width = content_width.saturating_sub(current_width);
                        let text_to_print = truncate_to_width(text, remaining_width);
                        queue!(
                            stdout,
                            SetAttribute(Attribute::Reset),
                            SetBackgroundColor(line_bg),
                            SetForegroundColor(fg),
                            Print(&text_to_print),
                        )?;
                        current_width += text_to_print.width();
                    }
                    // Padding
                    let padding = content_width.saturating_sub(current_width);
                    if padding > 0 {
                        queue!(
                            stdout,
                            SetAttribute(Attribute::Reset),
                            SetBackgroundColor(line_bg),
                            Print(format!("{:width$}", "", width = padding)),
                        )?;
                    }
                }
            }
        }
        None => {
            // Blank line (no corresponding line on this side)
            // Show empty gutter and blank content
            let blank_gutter = format!("{:>width$} ", "~", width = gutter_width - 1);
            queue!(
                stdout,
                SetBackgroundColor(colors.bg),
                SetForegroundColor(Color::DarkGrey),
                Print(&blank_gutter),
                Print(format!("{:width$}", "", width = content_width)),
            )?;
        }
    }

    queue!(stdout, ResetColor)?;
    Ok(())
}

/// Get background colors for diff line based on change type.
fn get_diff_colors(
    change: &LineChange,
    is_cursor_line: bool,
    is_left_pane: bool,
    colors: &ColorScheme,
) -> (Color, Color, Color) {
    let base_bg = if is_cursor_line {
        colors.selected_bg
    } else {
        colors.bg
    };

    match change {
        LineChange::Added => {
            if is_left_pane {
                // Use cursor-aware color when cursor is on added line
                let bg = if is_cursor_line {
                    colors.diff_added_selected_bg
                } else {
                    colors.diff_added_bg
                };
                (bg, colors.diff_added_word_bg, colors.diff_removed_word_bg)
            } else {
                (base_bg, colors.diff_added_word_bg, colors.diff_removed_word_bg)
            }
        }
        LineChange::Removed => {
            if is_left_pane {
                (base_bg, colors.diff_added_word_bg, colors.diff_removed_word_bg)
            } else {
                // Use cursor-aware color when cursor is on removed line (right pane)
                let bg = if is_cursor_line {
                    colors.diff_removed_selected_bg
                } else {
                    colors.diff_removed_bg
                };
                (bg, colors.diff_added_word_bg, colors.diff_removed_word_bg)
            }
        }
        LineChange::Modified { .. } => {
            (base_bg, colors.diff_added_word_bg, colors.diff_removed_word_bg)
        }
        LineChange::Unchanged => {
            (base_bg, colors.diff_added_word_bg, colors.diff_removed_word_bg)
        }
    }
}

/// Render word-level diff with highlighting.
fn render_word_diff(
    stdout: &mut impl Write,
    words: &[WordChange],
    leading_ws: &str,
    line_bg: Color,
    word_added_bg: Color,
    word_removed_bg: Color,
    is_left_pane: bool,
    content_width: usize,
) -> io::Result<()> {
    let mut current_width = 0;

    // Render leading whitespace first
    if !leading_ws.is_empty() && current_width < content_width {
        // Use truncate_to_width for proper wide character handling
        use crate::text::truncate_to_width;
        let ws_to_print = truncate_to_width(leading_ws, content_width);
        queue!(
            stdout,
            SetBackgroundColor(line_bg),
            Print(&ws_to_print),
        )?;
        current_width += ws_to_print.width();
    }

    let mut first_word = true;

    for word in words {
        if current_width >= content_width {
            break;
        }

        // Determine which words to show on each pane
        let should_show = match word.change_type {
            ChangeType::Unchanged => true,
            ChangeType::Added => is_left_pane,    // Added words only on left (working)
            ChangeType::Removed => !is_left_pane, // Removed words only on right (HEAD)
        };

        if !should_show {
            continue;
        }

        // Add space between words (except first)
        if !first_word && current_width < content_width {
            queue!(
                stdout,
                SetBackgroundColor(line_bg),
                Print(" "),
            )?;
            current_width += 1;
        }
        first_word = false;

        // Determine word background
        let word_bg = match word.change_type {
            ChangeType::Unchanged => line_bg,
            ChangeType::Added => word_added_bg,
            ChangeType::Removed => word_removed_bg,
        };

        // Print word with appropriate background
        // Use truncate_to_width for proper wide character handling
        use crate::text::truncate_to_width;
        let remaining_width = content_width.saturating_sub(current_width);
        let text_to_print = truncate_to_width(&word.text, remaining_width);

        queue!(
            stdout,
            SetBackgroundColor(word_bg),
            Print(&text_to_print),
        )?;
        current_width += text_to_print.width();
    }

    // Padding
    let padding = content_width.saturating_sub(current_width);
    if padding > 0 {
        queue!(
            stdout,
            SetBackgroundColor(line_bg),
            Print(format!("{:width$}", "", width = padding)),
        )?;
    }

    Ok(())
}

/// Render annotation area in diff mode.
fn render_diff_annotation_area(
    stdout: &mut impl Write,
    lines: &[Line],
    cursor_line: usize,
    editor_state: &EditorState,
    annotation_scroll: usize,
    colors: &ColorScheme,
    width: u16,
    annotation_start: u16,
) -> io::Result<()> {
    // Top border
    queue!(
        stdout,
        MoveTo(0, annotation_start),
        SetBackgroundColor(colors.annotation_window_bg),
        SetForegroundColor(colors.annotation_window_fg),
        Print(format!("╔{}╗", "═".repeat(width as usize - 2))),
        ResetColor
    )?;

    // Get annotation text based on editor state
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
                lines[cursor_line]
                    .annotation
                    .clone()
                    .unwrap_or_else(|| "[No annotation - Press Enter to add]".to_string())
            } else {
                "[No annotation - Press Enter to add]".to_string()
            }
        }
    };

    // Wrap annotation text
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

        queue!(
            stdout,
            MoveTo(0, y_pos),
            SetBackgroundColor(colors.annotation_window_bg),
            SetForegroundColor(colors.annotation_window_fg),
            Print(format!(
                "║ {:width$} ║",
                display_line,
                width = max_annotation_width
            )),
            ResetColor
        )?;
    }

    // Bottom border
    queue!(
        stdout,
        MoveTo(0, annotation_start + 3),
        SetBackgroundColor(colors.annotation_window_bg),
        SetForegroundColor(colors.annotation_window_fg),
        Print(format!("╚{}╝", "═".repeat(width as usize - 2))),
        ResetColor
    )?;

    Ok(())
}

/// Render status bar in diff mode.
#[allow(clippy::too_many_arguments)]
fn render_diff_status_bar(
    stdout: &mut impl Write,
    editor_state: &EditorState,
    file_path: &Option<String>,
    modified: bool,
    cursor_line: usize,
    total_lines: usize,
    colors: &ColorScheme,
    width: u16,
    height: u16,
    status_message: Option<&str>,
    diff_available: bool,
) -> io::Result<()> {
    queue!(stdout, MoveTo(0, height - 1))?;

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
                .unwrap_or("[No Name]");
            let modified_flag = if modified { " [Modified]" } else { "" };

            // Build the left part: DIFF indicator, filename and line info
            let left_part = format!(
                " DIFF | {}{} | Line {}/{}",
                filename, modified_flag, cursor_line + 1, total_lines
            );

            // Render left part with normal status colors
            queue!(
                stdout,
                SetBackgroundColor(colors.status_bg),
                SetForegroundColor(colors.status_fg),
                Print(&left_part),
            )?;

            // If diff is available, show the orange indicator (for exit diff) with a space before it
            if diff_available {
                let diff_indicator = " ^D Exit Diff ";
                queue!(
                    stdout,
                    SetBackgroundColor(colors.status_bg),
                    Print(" "),
                    SetBackgroundColor(colors.diff_indicator_bg),
                    SetForegroundColor(colors.diff_indicator_fg),
                    Print(diff_indicator),
                )?;
            }

            // Continue with the rest of the shortcuts
            let shortcuts = " ^G Help  ^X Exit  ^O Save  ^W Search  ^T Theme  Del/Bksp Del  ^N/^P Jump";
            let current_len = left_part.len() + if diff_available { 15 } else { 0 }; // " " + " ^D Exit Diff "
            let remaining_width = (width as usize).saturating_sub(current_len + 1);
            let shortcuts_truncated: String = shortcuts.chars().take(remaining_width).collect();

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
            let search_status = format!("Search: {}█  Enter: Next  Esc: Cancel", query);
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

/// Position cursor in annotation area for diff mode.
fn position_diff_cursor(
    stdout: &mut impl Write,
    buffer: &str,
    cursor_pos: usize,
    annotation_scroll: usize,
    annotation_start: u16,
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
        (
            found_line,
            found_col.min(
                wrapped_annotation
                    .get(found_line)
                    .map(|l| l.chars().count())
                    .unwrap_or(0),
            ),
        )
    } else {
        (0, 0)
    };

    let cursor_screen_line =
        if cursor_line >= annotation_scroll && cursor_line < annotation_scroll + 2 {
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
    let cursor_x = 2 + cursor_visual_col;

    queue!(stdout, MoveTo(cursor_x as u16, cursor_screen_line), Show)?;

    Ok(())
}

/// Render help overlay in diff mode.
fn render_diff_help_overlay(
    stdout: &mut impl Write,
    colors: &ColorScheme,
    width: u16,
    height: u16,
) -> io::Result<()> {
    // Center the box
    let box_width = 50;
    let box_height = 18;
    let start_x = (width.saturating_sub(box_width)) / 2;
    let start_y = (height.saturating_sub(box_height)) / 2;

    // Draw background
    for y in 0..box_height {
        if start_y + y >= height {
            break;
        }
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

    for y in 1..box_height - 1 {
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
        " HELP MENU (DIFF MODE) ",
        "",
        " ^N / ^P    Next / Prev Annotation",
        " Del/Bksp   Delete Annotation",
        " Enter      Add / Edit Annotation",
        " ^W         Search",
        " ^D / Esc   Exit Diff View",
        " ^T         Toggle Theme",
        " ^O         Save File",
        " ^X         Exit",
        " ^G         Toggle Help",
        "",
        " Arrow Keys Navigation",
        " PgUp/PgDn  Page Navigation",
        "",
        " Press Any Key to Close",
    ];

    for (i, cmd) in commands.iter().enumerate() {
        if i as u16 >= box_height - 2 {
            break;
        }
        queue!(
            stdout,
            MoveTo(start_x + 2, start_y + 1 + i as u16),
            Print(cmd),
        )?;
    }

    queue!(stdout, ResetColor)?;

    Ok(())
}
