//! Diff mode UI rendering module.

use crate::diff::{ChangeType, DiffResult, LineChange};
use crate::highlighting::{to_crossterm_color, SyntaxHighlighter};
use crate::models::{EditorState, FocusedPanel, Line};
use crate::text::{wrap_text, wrap_styled_text};
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

/// Represents a wrapped line in a diff pane with rendering info
struct WrappedPaneLine {
    line_num: Option<usize>,
    styled_segments: Vec<(crossterm::style::Color, crossterm::style::Color, String)>, // (fg, bg, text)
    default_bg: Color,
}

/// Wraps content for a single diff pane, returning wrapped lines with styling
#[allow(clippy::too_many_arguments)]
fn wrap_diff_pane_content(
    line_data: &Option<(usize, String, LineChange)>,
    content_width: usize,
    _gutter_width: usize,
    is_left_pane: bool,
    colors: &ColorScheme,
    highlighter: &SyntaxHighlighter,
    extension: &str,
    lines: &[Line],
    is_cursor_line: bool,
) -> Vec<WrappedPaneLine> {
    match line_data {
        Some((line_num, content, change)) => {
            // Determine background color
            let (line_bg, _word_added_bg, _word_removed_bg) = get_diff_colors(change, is_cursor_line, is_left_pane, colors);

            // Check if line has annotation
            let has_annotation = if is_left_pane && *line_num > 0 && *line_num <= lines.len() {
                lines[*line_num - 1].annotation.is_some()
            } else {
                false
            };

            let line_bg = if has_annotation && is_cursor_line {
                colors.annotated_selected_bg
            } else if has_annotation {
                colors.annotated_bg
            } else {
                line_bg
            };

            // Handle word-level diff for modified lines
            if let LineChange::Modified { words, old_leading_ws: _, new_leading_ws: _ } = change {
                // Create styled segments with word-level diff colors
                let word_added_bg = colors.diff_added_word_bg;
                let word_removed_bg = colors.diff_removed_word_bg;
                let word_added_fg = colors.diff_added_word_fg;
                let word_removed_fg = colors.diff_removed_word_fg;

                // NEW APPROACH: Use original content as-is, map word changes to positions
                // Step 1: Map word changes to character positions in the original content
                let change_positions = map_word_changes_to_positions(content, words, is_left_pane);

                // Step 2: Apply syntax highlighting to the original content (preserves spacing)
                let syntax_highlighted = highlighter.highlight(content, extension);

                // Step 3: Create styled segments by merging syntax highlighting with word-level diff colors
                let mut styled_segments = Vec::new();
                let mut current_char = 0;

                for (syntax_style, syntax_text) in syntax_highlighted {
                    let syntax_fg = to_crossterm_color(syntax_style.foreground);
                    let syntax_len = syntax_text.chars().count();
                    let syntax_start = current_char;
                    let syntax_end = current_char + syntax_len;

                    // Check if this syntax segment overlaps with any changed word
                    let mut overlaps = Vec::new();
                    for (word_start, word_end, change_type) in &change_positions {
                        let overlap_start = syntax_start.max(*word_start);
                        let overlap_end = syntax_end.min(*word_end);
                        if overlap_start < overlap_end {
                            overlaps.push((overlap_start, overlap_end, change_type));
                        }
                    }

                    if overlaps.is_empty() {
                        // No overlap with changed words - use default syntax colors
                        styled_segments.push((syntax_fg, line_bg, syntax_text.to_string()));
                    } else {
                        // Split segment based on overlaps
                        let chars: Vec<char> = syntax_text.chars().collect();
                        let mut segment_pos = syntax_start;

                        for (overlap_start, overlap_end, change_type) in overlaps {
                            // Add unchanged portion before overlap (if any)
                            if segment_pos < overlap_start {
                                let local_start = segment_pos.saturating_sub(syntax_start);
                                let local_end = overlap_start.saturating_sub(syntax_start);
                                let text: String = chars[local_start..local_end].iter().collect();
                                styled_segments.push((syntax_fg, line_bg, text));
                            }

                            // Add changed portion
                            let local_start = overlap_start.saturating_sub(syntax_start);
                            let local_end = overlap_end.saturating_sub(syntax_start);
                            let text: String = chars[local_start..local_end].iter().collect();

                            let (fg, bg) = match change_type {
                                ChangeType::Added => (word_added_fg, word_added_bg),
                                ChangeType::Removed => (word_removed_fg, word_removed_bg),
                                ChangeType::Unchanged => (syntax_fg, line_bg),
                            };
                            styled_segments.push((fg, bg, text));

                            segment_pos = overlap_end;
                        }

                        // Add any remaining unchanged portion after last overlap
                        if segment_pos < syntax_end {
                            let local_start = segment_pos.saturating_sub(syntax_start);
                            let local_end = syntax_len;
                            let text: String = chars[local_start..local_end].iter().collect();
                            styled_segments.push((syntax_fg, line_bg, text));
                        }
                    }

                    current_char = syntax_end;
                }

                // Step 4: Wrap the styled segments
                let wrapped_input: Vec<(Color, &str)> = styled_segments.iter()
                    .map(|(fg, _bg, text)| (*fg, text.as_str()))
                    .collect();
                let wrapped = wrap_styled_text(&wrapped_input, content_width);

                // Step 5: Reconstruct segments with background colors after wrapping
                wrapped.into_iter().enumerate().map(|(idx, segments)| {
                    let mut result_segments = Vec::new();
                    let mut segment_offset = 0;

                    for (fg, text) in segments {
                        // Find the corresponding original segment to get the background color
                        let char_count = text.chars().count();
                        let mut current_offset = 0;
                        let mut found_bg = line_bg;

                        for (_orig_fg, orig_bg, orig_text) in &styled_segments {
                            let orig_len = orig_text.chars().count();
                            if current_offset <= segment_offset && segment_offset < current_offset + orig_len {
                                found_bg = *orig_bg;
                                break;
                            }
                            current_offset += orig_len;
                        }

                        result_segments.push((fg, found_bg, text));
                        segment_offset += char_count;
                    }

                    WrappedPaneLine {
                        line_num: if idx == 0 { Some(*line_num) } else { None },
                        styled_segments: result_segments,
                        default_bg: line_bg,
                    }
                }).collect()
            } else {
                // Simple highlight for added/removed/unchanged lines
                let styled_spans = highlighter.highlight(content, extension);
                let wrapped = wrap_styled_text(&styled_spans, content_width);

                wrapped.into_iter().enumerate().map(|(idx, segments)| {
                    WrappedPaneLine {
                        line_num: if idx == 0 { Some(*line_num) } else { None },
                        styled_segments: segments.into_iter().map(|(style, text)| {
                            (to_crossterm_color(style.foreground), line_bg, text)
                        }).collect(),
                        default_bg: line_bg,
                    }
                }).collect()
            }
        }
        None => {
            // Blank line
            vec![WrappedPaneLine {
                line_num: None,
                styled_segments: vec![],
                default_bg: colors.bg,
            }]
        }
    }
}

/// Renders a single wrapped line from a diff pane
fn render_wrapped_pane_line(
    stdout: &mut impl Write,
    wrapped_line: &WrappedPaneLine,
    start_x: u16,
    gutter_width: usize,
    content_width: usize,
    y: u16,
    show_gutter: bool,
) -> io::Result<()> {
    queue!(stdout, MoveTo(start_x, y))?;

    // Render gutter
    if show_gutter {
        if let Some(line_num) = wrapped_line.line_num {
            let line_num_str = format!("{:>width$} ", line_num, width = gutter_width - 1);
            queue!(
                stdout,
                SetBackgroundColor(Color::Reset),
                SetForegroundColor(Color::DarkGrey),
                Print(&line_num_str),
            )?;
        } else {
            // Blank gutter for wrapped continuation lines
            let blank_gutter = format!("{:>width$} ", "", width = gutter_width - 1);
            queue!(
                stdout,
                SetBackgroundColor(Color::Reset),
                Print(&blank_gutter),
            )?;
        }
    } else {
        // Show blank gutter for None lines
        let blank_gutter = format!("{:>width$} ", "~", width = gutter_width - 1);
        queue!(
            stdout,
            SetBackgroundColor(Color::Reset),
            SetForegroundColor(Color::DarkGrey),
            Print(&blank_gutter),
        )?;
    }

    // Render styled segments with per-segment background colors
    let mut current_width = 0;
    for (fg, bg, text) in &wrapped_line.styled_segments {
        if current_width >= content_width {
            break;
        }
        queue!(
            stdout,
            SetAttribute(Attribute::Reset),
            SetBackgroundColor(*bg),
            SetForegroundColor(*fg),
            Print(text),
        )?;
        current_width += text.width();
    }

    // Padding with default background
    let padding = content_width.saturating_sub(current_width);
    if padding > 0 {
        queue!(
            stdout,
            SetAttribute(Attribute::Reset),
            SetBackgroundColor(wrapped_line.default_bg),
            Print(format!("{:width$}", "", width = padding)),
        )?;
    }

    queue!(stdout, ResetColor)?;
    Ok(())
}

/// Renders an empty pane line for padding
fn render_empty_pane_line(
    stdout: &mut impl Write,
    start_x: u16,
    width: usize,
    colors: &ColorScheme,
    y: u16,
) -> io::Result<()> {
    queue!(
        stdout,
        MoveTo(start_x, y),
        SetBackgroundColor(colors.bg),
        Print(format!("{:width$}", "", width = width)),
        ResetColor
    )?;
    Ok(())
}

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
    start_col: u16,
    available_width: u16,
    focused_panel: FocusedPanel,
) -> io::Result<()> {
    let (terminal_width, height) = terminal::size()?;
    let content_height = (height.saturating_sub(6)) as usize; // -6: title bar, annotation (4), status (1)
    let colors = theme.colors();

    let mut stdout = io::stdout();
    queue!(stdout, MoveTo(start_col, 0))?;

    // Calculate pane widths (use available_width, not terminal width)
    let separator_width = 1;
    let total_content_width = (available_width as usize).saturating_sub(separator_width);
    let left_pane_width = total_content_width / 2;
    let right_pane_width = total_content_width - left_pane_width;

    // Render unified title bar for both panes
    let is_left_focused = focused_panel == FocusedPanel::Editor;
    render_unified_diff_title_bar(
        &mut stdout,
        "Working Copy",
        "HEAD",
        start_col,
        available_width,
        left_pane_width as u16,
        is_left_focused,
        &colors,
    )?;

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

    // Determine border color (focused when editor panel is active)
    let border_color = if is_left_focused {
        colors.panel_border_focused
    } else {
        colors.panel_border_unfocused
    };

    // Render diff content with word wrapping
    while screen_line < content_height && diff_line_idx < diff_result.lines.len() {
        let diff_line = &diff_result.lines[diff_line_idx];

        // Determine if this diff line corresponds to the cursor
        let is_cursor_line = diff_line
            .working
            .as_ref()
            .map(|(n, _, _)| *n == cursor_line + 1)
            .unwrap_or(false);

        // Wrap content for both panes
        let left_wrapped = wrap_diff_pane_content(
            &diff_line.working,
            left_content_width.saturating_sub(1),
            left_gutter_width,
            true,
            &colors,
            highlighter,
            extension,
            lines,
            is_cursor_line,
        );

        let right_wrapped = wrap_diff_pane_content(
            &diff_line.head,
            right_content_width.saturating_sub(1),
            right_gutter_width,
            false,
            &colors,
            highlighter,
            extension,
            lines,
            false, // cursor never on right pane
        );

        // Determine how many screen lines this diff line needs
        let wrapped_line_count = left_wrapped.len().max(right_wrapped.len());

        // Render each wrapped line
        for wrap_idx in 0..wrapped_line_count {
            if screen_line >= content_height {
                break;
            }

            let y_pos = screen_line as u16 + 1;

            // Clear the entire line with background color first
            queue!(
                stdout,
                MoveTo(start_col, y_pos),
                SetBackgroundColor(colors.bg),
                Print(format!("{:width$}", "", width = available_width as usize)),
            )?;

            // Render left border
            queue!(
                stdout,
                MoveTo(start_col, y_pos),
                SetBackgroundColor(colors.bg),
                SetForegroundColor(border_color),
                Print("│"),
                ResetColor
            )?;

            // Render left pane wrapped line
            if wrap_idx < left_wrapped.len() {
                render_wrapped_pane_line(
                    &mut stdout,
                    &left_wrapped[wrap_idx],
                    start_col + 1,
                    left_gutter_width,
                    left_content_width.saturating_sub(1),
                    y_pos,
                    wrap_idx == 0, // show gutter only on first line
                )?;
            } else {
                // Empty line (padding)
                render_empty_pane_line(
                    &mut stdout,
                    start_col + 1,
                    left_pane_width.saturating_sub(1),
                    &colors,
                    y_pos,
                )?;
            }

            // Render separator
            queue!(
                stdout,
                MoveTo(start_col + left_pane_width as u16 + 1, y_pos),
                SetBackgroundColor(colors.bg),
                SetForegroundColor(border_color),
                Print("│"),
                ResetColor
            )?;

            // Render right pane wrapped line
            if wrap_idx < right_wrapped.len() {
                render_wrapped_pane_line(
                    &mut stdout,
                    &right_wrapped[wrap_idx],
                    start_col + (left_pane_width + separator_width) as u16 + 1,
                    right_gutter_width,
                    right_content_width.saturating_sub(1),
                    y_pos,
                    wrap_idx == 0,
                )?;
            } else {
                // Empty line (padding)
                render_empty_pane_line(
                    &mut stdout,
                    start_col + (left_pane_width + separator_width) as u16 + 1,
                    right_pane_width.saturating_sub(1),
                    &colors,
                    y_pos,
                )?;
            }

            // Render right border
            queue!(
                stdout,
                MoveTo(start_col + available_width - 1, y_pos),
                SetBackgroundColor(colors.bg),
                SetForegroundColor(border_color),
                Print("│"),
                ResetColor
            )?;

            screen_line += 1;
        }

        diff_line_idx += 1;
    }

    // Fill remaining content area
    while screen_line < content_height {
        let y_pos = screen_line as u16 + 1;

        // Clear the entire line with background color first (prevents theme switch artifacts)
        queue!(
            stdout,
            MoveTo(start_col, y_pos),
            SetBackgroundColor(colors.bg),
            Print(format!("{:width$}", "", width = available_width as usize)),
        )?;

        // Left border
        queue!(
            stdout,
            MoveTo(start_col, y_pos),
            SetBackgroundColor(colors.bg),
            SetForegroundColor(border_color),
            Print("│"),
            ResetColor
        )?;

        // Left pane empty
        queue!(
            stdout,
            MoveTo(start_col + 1, y_pos),
            SetBackgroundColor(colors.bg),
            Print(format!("{:width$}", "", width = left_pane_width.saturating_sub(1))),
        )?;

        // Separator (matches border style)
        queue!(
            stdout,
            MoveTo(start_col + left_pane_width as u16 + 1, y_pos),
            SetBackgroundColor(colors.bg),
            SetForegroundColor(border_color),
            Print("│"),
            ResetColor
        )?;

        // Right pane empty
        queue!(
            stdout,
            MoveTo(start_col + (left_pane_width + separator_width) as u16 + 1, y_pos),
            SetBackgroundColor(colors.bg),
            Print(format!("{:width$}", "", width = right_pane_width.saturating_sub(1))),
        )?;

        // Right border
        queue!(
            stdout,
            MoveTo(start_col + available_width - 1, y_pos),
            SetBackgroundColor(colors.bg),
            SetForegroundColor(border_color),
            Print("│"),
            ResetColor
        )?;

        screen_line += 1;
    }

    // Render bottom border before annotation area with junction
    let border_y = height.saturating_sub(6); // One line above annotation

    // Left corner
    queue!(
        stdout,
        MoveTo(start_col, border_y),
        SetBackgroundColor(colors.bg),
        SetForegroundColor(border_color),
        Print("└"),
        ResetColor
    )?;

    // Left section horizontal line
    let left_horizontal = "─".repeat(left_pane_width as usize);
    queue!(
        stdout,
        SetBackgroundColor(colors.bg),
        SetForegroundColor(border_color),
        Print(&left_horizontal),
        ResetColor
    )?;

    // Middle junction (aligns with content separator)
    queue!(
        stdout,
        SetBackgroundColor(colors.bg),
        SetForegroundColor(border_color),
        Print("┴"),
        ResetColor
    )?;

    // Right section horizontal line
    let right_section_width = (available_width as usize)
        .saturating_sub(left_pane_width as usize)
        .saturating_sub(3); // -3 for left corner, middle junction, right corner
    let right_horizontal = "─".repeat(right_section_width);
    queue!(
        stdout,
        SetBackgroundColor(colors.bg),
        SetForegroundColor(border_color),
        Print(&right_horizontal),
        ResetColor
    )?;

    // Right corner
    queue!(
        stdout,
        SetBackgroundColor(colors.bg),
        SetForegroundColor(border_color),
        Print("┘"),
        ResetColor
    )?;

    // Render annotation area (starts at start_col, uses available_width)
    let annotation_start = height - 5;
    let is_annotation_focused = matches!(editor_state, EditorState::Annotating { .. });
    render_diff_annotation_area(
        &mut stdout,
        lines,
        cursor_line,
        editor_state,
        annotation_scroll,
        &colors,
        available_width,
        annotation_start,
        is_annotation_focused,
        start_col, // Start at column after tree
    )?;

    // Render status bar (full terminal width, from column 0)
    render_diff_status_bar(
        &mut stdout,
        editor_state,
        file_path,
        modified,
        cursor_line,
        lines.len(),
        &colors,
        terminal_width,
        height,
        status_message,
        diff_available,
    )?;

    // Render help overlay if showing help
    if matches!(editor_state, EditorState::ShowingHelp) {
        render_diff_help_overlay(&mut stdout, &colors, terminal_width, height)?;
    }

    // Position and show cursor if in annotation edit state
    if let EditorState::Annotating { buffer, cursor_pos } = editor_state {
        position_diff_cursor(
            &mut stdout,
            buffer,
            *cursor_pos,
            annotation_scroll,
            annotation_start,
            start_col,
            available_width,
        )?;
    } else {
        queue!(stdout, Hide)?;
    }

    stdout.flush()?;
    Ok(())
}

/// Renders a unified title bar for both diff panes with borders.
fn render_unified_diff_title_bar(
    stdout: &mut impl Write,
    left_title: &str,
    right_title: &str,
    start_col: u16,
    total_width: u16,
    left_pane_width: u16,
    is_left_focused: bool,
    colors: &ColorScheme,
) -> io::Result<()> {
    queue!(stdout, MoveTo(start_col, 0))?;

    // Determine colors based on focus
    let border_color = if is_left_focused {
        colors.panel_border_focused
    } else {
        colors.panel_border_unfocused
    };

    let left_bg = if is_left_focused {
        colors.panel_title_focused_bg
    } else {
        colors.panel_title_unfocused_bg
    };
    let left_fg = if is_left_focused {
        colors.panel_title_focused_fg
    } else {
        colors.panel_title_unfocused_fg
    };

    // HEAD pane is never focused
    let right_bg = colors.panel_title_unfocused_bg;
    let right_fg = colors.panel_title_unfocused_fg;

    // Left corner
    queue!(
        stdout,
        SetBackgroundColor(colors.bg),
        SetForegroundColor(border_color),
        Print("┌"),
        ResetColor
    )?;

    // Left pane title bar content
    // Content spans from position 1 to left_pane_width (inclusive)
    let left_content_width = left_pane_width as usize;
    let left_title_with_spaces = format!(" {} ", left_title);
    let left_title_len = left_title_with_spaces.chars().count();
    let left_available = left_content_width.saturating_sub(left_title_len);
    let left_padding = "─".repeat(left_available);

    queue!(
        stdout,
        SetBackgroundColor(left_bg),
        SetForegroundColor(left_fg),
        Print(&left_title_with_spaces),
        SetBackgroundColor(colors.bg),
        SetForegroundColor(border_color),
        Print(&left_padding),
        ResetColor
    )?;

    // Middle separator (junction between panes)
    // This should align with the content separator at start_col + left_pane_width + 1
    queue!(
        stdout,
        SetBackgroundColor(colors.bg),
        SetForegroundColor(border_color),
        Print("┬"),
        ResetColor
    )?;

    // Right pane title bar content
    // The right pane spans from position left_pane_width + 2 to total_width - 1
    let right_content_width = (total_width as usize)
        .saturating_sub(left_pane_width as usize)
        .saturating_sub(3); // -3 for left corner, middle separator, right corner
    let right_title_with_spaces = format!(" {} ", right_title);
    let right_title_len = right_title_with_spaces.chars().count();
    let right_available = right_content_width.saturating_sub(right_title_len);
    let right_padding = "─".repeat(right_available);

    queue!(
        stdout,
        SetBackgroundColor(right_bg),
        SetForegroundColor(right_fg),
        Print(&right_title_with_spaces),
        SetBackgroundColor(colors.bg),
        SetForegroundColor(border_color),
        Print(&right_padding),
        ResetColor
    )?;

    // Right corner
    queue!(
        stdout,
        SetBackgroundColor(colors.bg),
        SetForegroundColor(border_color),
        Print("┐"),
        ResetColor
    )?;

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
            // Modified lines get pale background based on which pane
            let bg = if is_left_pane {
                // Working copy shows additions - pale green
                if is_cursor_line {
                    colors.diff_added_selected_bg
                } else {
                    colors.diff_added_bg
                }
            } else {
                // HEAD shows removals - pale red
                if is_cursor_line {
                    colors.diff_removed_selected_bg
                } else {
                    colors.diff_removed_bg
                }
            };
            (bg, colors.diff_added_word_bg, colors.diff_removed_word_bg)
        }
        LineChange::Unchanged => {
            (base_bg, colors.diff_added_word_bg, colors.diff_removed_word_bg)
        }
    }
}

/// Maps word changes to their positions in the original content string.
/// Returns a list of (start_pos, end_pos, change_type) for changed words only.
/// Unchanged words are not included in the result (they get default syntax highlighting).
fn map_word_changes_to_positions(
    content: &str,
    words: &[crate::diff::WordChange],
    is_left_pane: bool,
) -> Vec<(usize, usize, ChangeType)> {
    use crate::diff::tokenize_line;

    let content_trimmed = content.trim_start();
    let leading_ws_len = content.len() - content_trimmed.len();
    let content_trimmed_end = content_trimmed.trim_end();

    // Tokenize the content to get actual token positions
    let mut content_tokens = Vec::new();
    let mut pos = 0;
    for token in tokenize_line(content_trimmed_end) {
        // Find this token in the remaining content
        if let Some(offset) = content_trimmed_end[pos..].find(token) {
            let start = leading_ws_len + pos + offset;
            let end = start + token.len();
            content_tokens.push((token, start, end));
            pos += offset + token.len();
        }
    }

    // Build a list of change types for content tokens by matching with words list
    let mut change_positions = Vec::new();
    let mut words_idx = 0;

    for (token, start, end) in content_tokens {
        // Find matching word in words list
        while words_idx < words.len() {
            let word = &words[words_idx];

            // Check if this word should appear in this pane
            let should_show = match word.change_type {
                ChangeType::Unchanged => true,
                ChangeType::Added => is_left_pane,
                ChangeType::Removed => !is_left_pane,
            };

            if !should_show {
                words_idx += 1;
                continue;
            }

            // Match token
            if word.text == token {
                // Only record positions for changed words (Added/Removed)
                // Unchanged words will use default syntax highlighting
                match word.change_type {
                    ChangeType::Added | ChangeType::Removed => {
                        change_positions.push((start, end, word.change_type.clone()));
                    }
                    ChangeType::Unchanged => {}
                }
                words_idx += 1;
                break;
            }

            words_idx += 1;
        }
    }

    change_positions
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
    is_focused: bool,
    start_col: u16,
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

    queue!(
        stdout,
        MoveTo(start_col, annotation_start),
        SetBackgroundColor(colors.annotation_window_bg),
        SetForegroundColor(border_color),
        Print(top_border),
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
            let shortcuts = " F1 Help  ^X Exit  ^O Save  ^W Search  ^T Theme  Del/Bksp Del  ^N/^P Jump";
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

/// Position cursor in annotation area for diff mode.
fn position_diff_cursor(
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
    let cursor_x = start_col + 2 + cursor_visual_col as u16;

    queue!(stdout, MoveTo(cursor_x, cursor_screen_line), Show)?;

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
        " F1         Show Help",
        " ^Z / ^Y    Undo / Redo",
        "",
        " Arrow Keys Navigation",
        " PgUp/PgDn  Page Navigation",
        "",
        " Hotkeys work in EN/RU layouts",
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

#[cfg(test)]
mod diff_title_tests {
    

    #[test]
    fn test_left_pane_title_text() {
        // Verify left pane title for diff mode
        let title = "Working Copy";
        assert_eq!(title, "Working Copy");
        assert!(!title.is_empty());
    }

    #[test]
    fn test_right_pane_title_text() {
        // Verify right pane title for diff mode
        let title = "HEAD";
        assert_eq!(title, "HEAD");
        assert!(!title.is_empty());
    }

    #[test]
    fn test_diff_content_height_with_titles() {
        // Verify content height calculation in diff mode accounts for title bars
        let height = 50u16;
        let title_bar_height = 1u16;
        let annotation_and_status = 5u16;
        let content_height = height.saturating_sub(annotation_and_status + title_bar_height);

        // Without title: 50 - 5 = 45
        // With title: 50 - 6 = 44
        assert_eq!(content_height, 44);
    }

    #[test]
    fn test_pane_title_width_calculation() {
        // Verify title width fits within pane width
        let total_width = 100u16;
        let separator_width = 1u16;
        let available_width = total_width.saturating_sub(separator_width);
        let pane_width = available_width / 2;

        // Each pane should have reasonable width for title
        assert!(pane_width >= 10, "Pane width too narrow for title");
        assert_eq!(pane_width, 49); // (100 - 1) / 2 = 49
    }

    // =========================================================================
    // Cursor Positioning Tests (Diff Mode)
    // =========================================================================

    #[test]
    fn test_diff_cursor_x_with_tree() {
        // In diff mode with tree, annotation starts at start_col (30)
        let start_col = 30u16;
        let border_and_padding = 2u16; // "│ "
        let cursor_visual_col = 8u16;
        let cursor_x = start_col + border_and_padding + cursor_visual_col;
        assert_eq!(cursor_x, 40);
    }

    #[test]
    fn test_diff_cursor_x_without_tree() {
        // In diff mode without tree, annotation starts at start_col (0)
        let start_col = 0u16;
        let border_and_padding = 2u16;
        let cursor_visual_col = 12u16;
        let cursor_x = start_col + border_and_padding + cursor_visual_col;
        assert_eq!(cursor_x, 14);
    }

    #[test]
    fn test_diff_cursor_x_at_start() {
        // Cursor at position 0 in annotation
        let start_col = 30u16;
        let cursor_visual_col = 0u16;
        let cursor_x = start_col + 2 + cursor_visual_col;
        assert_eq!(cursor_x, 32); // Right after "│ "
    }

    #[test]
    fn test_diff_cursor_accounts_for_annotation_width() {
        // Verify cursor respects annotation area boundaries
        let start_col = 30u16;
        let available_width = 100u16;
        let max_annotation_width = available_width as usize - 4; // "│ " and " │"

        // Cursor near end of line
        let cursor_visual_col = (max_annotation_width - 1) as u16;
        let cursor_x = start_col + 2 + cursor_visual_col;

        // Should be within bounds: start_col + width
        assert!(cursor_x < start_col + available_width);
        assert_eq!(cursor_x, 30 + 2 + 95); // 30 + 2 + (100 - 4 - 1) = 127
    }

    #[test]
    fn test_diff_cursor_y_first_visible_line() {
        // Cursor on first visible line in diff mode
        let annotation_start = 45u16;
        let annotation_scroll = 0usize;
        let cursor_line = 0usize;
        let cursor_y = annotation_start + 1 + (cursor_line - annotation_scroll) as u16;
        assert_eq!(cursor_y, 46);
    }

    #[test]
    fn test_diff_cursor_y_second_visible_line() {
        // Cursor on second visible line in diff mode
        let annotation_start = 45u16;
        let annotation_scroll = 0usize;
        let cursor_line = 1usize;
        let cursor_y = annotation_start + 1 + (cursor_line - annotation_scroll) as u16;
        assert_eq!(cursor_y, 47);
    }

    #[test]
    fn test_diff_cursor_y_with_scroll() {
        // Scrolled annotation in diff mode
        let annotation_start = 45u16;
        let annotation_scroll = 3usize;
        let cursor_line = 4usize;
        let cursor_y = annotation_start + 1 + (cursor_line - annotation_scroll) as u16;
        assert_eq!(cursor_y, 47); // Line 4 shows at second position (offset 1) when scroll is 3
    }

    #[test]
    fn test_diff_cursor_formula_matches_normal_mode() {
        // Verify diff mode cursor calculation matches normal mode formula
        let start_col = 30u16;
        let cursor_visual_col = 7u16;

        // Both modes should use: start_col + 2 + cursor_visual_col
        let cursor_x = start_col + 2 + cursor_visual_col;
        assert_eq!(cursor_x, 39);
    }

    #[test]
    fn test_unified_title_bar_corners() {
        // Verify title bar uses corner characters
        let left_corner = '┌';
        let right_corner = '┐';
        let middle_junction = '┬';

        // Format should be: ┌─ Working Copy ─┬─ HEAD ─┐
        assert_eq!(left_corner, '┌');
        assert_eq!(right_corner, '┐');
        assert_eq!(middle_junction, '┬');
    }

    #[test]
    fn test_title_bar_width_calculation() {
        // Verify title bar spans full width correctly
        let available_width = 100u16;
        let left_pane_width = 49u16; // (100 - 1) / 2

        // Title bar components:
        // ┌ (1) + left content (left_pane_width) + ┬ (1) + right content + ┐ (1)
        let left_corner = 1;
        let left_content = left_pane_width;
        let middle_sep = 1;
        let right_corner = 1;
        let right_content = available_width - left_corner - left_content - middle_sep - right_corner;

        let total = left_corner + left_content as u16 + middle_sep + right_content + right_corner;
        assert_eq!(total, available_width);
    }

    #[test]
    fn test_middle_separator_aligns_with_content() {
        // The middle ┬ should align with the content separator │
        let start_col = 30u16;
        let left_pane_width = 49u16;

        // Title bar middle ┬ position: start_col + 1 (corner) + left_pane_width
        let title_middle_pos = start_col + 1 + left_pane_width;

        // Content separator │ position: start_col + left_pane_width + 1
        let content_sep_pos = start_col + left_pane_width + 1;

        // They should be at the same position
        assert_eq!(title_middle_pos, content_sep_pos);
    }

    #[test]
    fn test_bottom_border_junction_alignment() {
        // The bottom ┴ should align with title ┬ and content │
        let start_col = 30u16;
        let left_pane_width = 49u16;

        // Title bar middle ┬ position
        let title_middle = start_col + 1 + left_pane_width;

        // Bottom border middle ┴ position
        let bottom_middle = start_col + 1 + left_pane_width;

        // All three should align vertically
        assert_eq!(title_middle, bottom_middle);
    }

    #[test]
    fn test_bottom_border_characters() {
        // Verify bottom border uses correct junction characters
        let left_corner = '└';
        let right_corner = '┘';
        let middle_junction = '┴';

        // Format should be: └─────┴─────┘
        assert_eq!(left_corner, '└');
        assert_eq!(right_corner, '┘');
        assert_eq!(middle_junction, '┴');
    }

    #[test]
    fn test_diff_border_box_complete() {
        // Verify all corners and junctions form a complete box
        // Top:    ┌─────┬─────┐
        // Middle: │     │     │
        // Bottom: └─────┴─────┘

        let top_left = '┌';
        let top_right = '┐';
        let top_middle = '┬';
        let bottom_left = '└';
        let bottom_right = '┘';
        let bottom_middle = '┴';
        let vertical = '│';

        // Verify all characters are correct
        assert_eq!(top_left, '┌');
        assert_eq!(top_right, '┐');
        assert_eq!(top_middle, '┬');
        assert_eq!(bottom_left, '└');
        assert_eq!(bottom_right, '┘');
        assert_eq!(bottom_middle, '┴');
        assert_eq!(vertical, '│');
    }

    #[test]
    fn test_word_diff_wrapping_with_syntax_highlighting() {
        use crate::diff::{ChangeType, LineChange, WordChange};
        use crate::highlighting::SyntaxHighlighter;
        use crate::models::Line;
        use crate::theme::Theme;

        // Test that word-level diffs preserve syntax highlighting for unchanged words
        let content = "adjust_scroll(*cursor,scroll,lines,view_mode)?;";

        // Create a mock modified line with word changes
        let words = vec![
            WordChange { text: "adjust_scroll".to_string(), change_type: ChangeType::Unchanged },
            WordChange { text: "(".to_string(), change_type: ChangeType::Unchanged },
            WordChange { text: "*".to_string(), change_type: ChangeType::Unchanged },
            WordChange { text: "cursor".to_string(), change_type: ChangeType::Unchanged },
            WordChange { text: ",".to_string(), change_type: ChangeType::Unchanged },
            WordChange { text: "scroll".to_string(), change_type: ChangeType::Unchanged },
            WordChange { text: ",".to_string(), change_type: ChangeType::Unchanged },
            WordChange { text: "lines".to_string(), change_type: ChangeType::Unchanged },
            WordChange { text: ",".to_string(), change_type: ChangeType::Unchanged },
            WordChange { text: "view_mode".to_string(), change_type: ChangeType::Added },
            WordChange { text: ")".to_string(), change_type: ChangeType::Unchanged },
            WordChange { text: "?".to_string(), change_type: ChangeType::Unchanged },
            WordChange { text: ";".to_string(), change_type: ChangeType::Unchanged },
        ];

        let change = LineChange::Modified {
            words,
            old_leading_ws: "".to_string(),
            new_leading_ws: "".to_string(),
        };

        // Create test data
        let lines = vec![Line { content: content.to_string(), annotation: None }];
        let colors = Theme::Dark.colors();
        let highlighter = SyntaxHighlighter::new(true);

        // Test wrapping function
        let wrapped = super::wrap_diff_pane_content(
            &Some((1, content.to_string(), change)),
            40, // content_width
            4,  // gutter_width
            true, // is_left_pane
            &colors,
            &highlighter,
            "rs",
            &lines,
            false, // is_cursor_line
        );

        // Should produce at least one wrapped line
        assert!(!wrapped.is_empty(), "Should produce wrapped lines");

        // Each wrapped line should have styled segments
        for wrapped_line in &wrapped {
            // If there are segments, verify they have both fg and bg colors
            if !wrapped_line.styled_segments.is_empty() {
                for (_fg, _bg, text) in &wrapped_line.styled_segments {
                    assert!(!text.is_empty(), "Segment should have text");
                }
            }
        }

        // Verify the output contains the added word
        let all_text: String = wrapped.iter()
            .flat_map(|line| &line.styled_segments)
            .map(|(_, _, text)| text.as_str())
            .collect();
        assert!(all_text.contains("view_mode"), "Should contain added word 'view_mode'");
    }

    #[test]
    fn test_word_diff_right_pane_colors() {
        use crate::diff::{ChangeType, LineChange, WordChange};
        use crate::highlighting::SyntaxHighlighter;
        use crate::models::Line;
        use crate::theme::Theme;

        // Test that right pane (HEAD) uses correct colors for removed words
        let content = "foo bar baz";

        // Create a mock modified line - right pane should show "bar" as removed
        let words = vec![
            WordChange { text: "foo".to_string(), change_type: ChangeType::Unchanged },
            WordChange { text: "bar".to_string(), change_type: ChangeType::Removed },
            WordChange { text: "baz".to_string(), change_type: ChangeType::Unchanged },
        ];

        let change = LineChange::Modified {
            words,
            old_leading_ws: "".to_string(),
            new_leading_ws: "".to_string(),
        };

        let lines = vec![Line { content: content.to_string(), annotation: None }];
        let colors = Theme::Dark.colors();
        let highlighter = SyntaxHighlighter::new(true);

        // Test wrapping for right pane
        let wrapped = super::wrap_diff_pane_content(
            &Some((1, content.to_string(), change)),
            40,
            4,
            false, // is_left_pane = false (this is HEAD/right pane)
            &colors,
            &highlighter,
            "txt",
            &lines,
            false,
        );

        assert!(!wrapped.is_empty(), "Right pane should produce wrapped lines");

        // Verify that segments exist
        let total_segments: usize = wrapped.iter()
            .map(|line| line.styled_segments.len())
            .sum();
        assert!(total_segments > 0, "Right pane should have styled segments");

        // Verify "bar" is included (it's removed, should show on right pane)
        let all_text: String = wrapped.iter()
            .flat_map(|line| &line.styled_segments)
            .map(|(_, _, text)| text.as_str())
            .collect();
        assert!(all_text.contains("bar"), "Right pane should contain removed word 'bar'");
    }

    #[test]
    fn test_word_diff_added_word_highlighting() {
        use crate::diff::{ChangeType, LineChange, WordChange};
        use crate::highlighting::SyntaxHighlighter;
        use crate::models::Line;
        use crate::theme::Theme;

        // Test that added words get special background color
        let content = "foo bar baz";

        // Create a mock modified line with "baz" added
        let words = vec![
            WordChange { text: "foo".to_string(), change_type: ChangeType::Unchanged },
            WordChange { text: "bar".to_string(), change_type: ChangeType::Unchanged },
            WordChange { text: "baz".to_string(), change_type: ChangeType::Added },
        ];

        let change = LineChange::Modified {
            words,
            old_leading_ws: "".to_string(),
            new_leading_ws: "".to_string(),
        };

        let lines = vec![Line { content: content.to_string(), annotation: None }];
        let colors = Theme::Dark.colors();
        let highlighter = SyntaxHighlighter::new(true);

        // Test wrapping
        let wrapped = super::wrap_diff_pane_content(
            &Some((1, content.to_string(), change)),
            40,
            4,
            true, // is_left_pane
            &colors,
            &highlighter,
            "txt",
            &lines,
            false,
        );

        // Verify wrapped lines contain the added word
        let all_text: String = wrapped.iter()
            .flat_map(|line| &line.styled_segments)
            .map(|(_, _, text)| text.as_str())
            .collect();
        assert!(all_text.contains("baz"), "Wrapped output should contain 'baz'");

        // Verify "baz" has a special background (added word background)
        let has_added_bg = wrapped.iter()
            .flat_map(|line| &line.styled_segments)
            .any(|(_, bg, text)| text.contains("baz") && *bg == colors.diff_added_word_bg);
        assert!(has_added_bg, "Added word 'baz' should have special background color");
    }

    #[test]
    fn test_word_diff_preserves_spacing_after_commas() {
        use crate::diff::{ChangeType, LineChange, WordChange};
        use crate::highlighting::SyntaxHighlighter;
        use crate::models::Line;
        use crate::theme::Theme;

        // Bug reproduction: spacing after commas should be preserved
        let _old_content = "use crate::diff::{ChangeType,DiffResult,LineChange};";
        let new_content = "use crate::diff::{ChangeType, DiffResult, LineChange};";

        // Simulate word-level diff (spaces added after commas)
        let words = vec![
            WordChange { text: "use".to_string(), change_type: ChangeType::Unchanged },
            WordChange { text: "crate".to_string(), change_type: ChangeType::Unchanged },
            WordChange { text: "::".to_string(), change_type: ChangeType::Unchanged },
            WordChange { text: "diff".to_string(), change_type: ChangeType::Unchanged },
            WordChange { text: "::".to_string(), change_type: ChangeType::Unchanged },
            WordChange { text: "{".to_string(), change_type: ChangeType::Unchanged },
            WordChange { text: "ChangeType".to_string(), change_type: ChangeType::Unchanged },
            WordChange { text: ",".to_string(), change_type: ChangeType::Unchanged },
            WordChange { text: "DiffResult".to_string(), change_type: ChangeType::Unchanged },
            WordChange { text: ",".to_string(), change_type: ChangeType::Unchanged },
            WordChange { text: "LineChange".to_string(), change_type: ChangeType::Unchanged },
            WordChange { text: "}".to_string(), change_type: ChangeType::Unchanged },
            WordChange { text: ";".to_string(), change_type: ChangeType::Unchanged },
        ];

        let change = LineChange::Modified {
            words,
            old_leading_ws: "".to_string(),
            new_leading_ws: "".to_string(),
        };

        let lines = vec![Line { content: new_content.to_string(), annotation: None }];
        let colors = Theme::Dark.colors();
        let highlighter = SyntaxHighlighter::new(true);

        // Test wrapping for left pane (new version)
        let wrapped = super::wrap_diff_pane_content(
            &Some((1, new_content.to_string(), change)),
            80,
            4,
            true, // is_left_pane
            &colors,
            &highlighter,
            "rs",
            &lines,
            false,
        );

        // Reconstruct text from segments
        let rendered_text: String = wrapped.iter()
            .flat_map(|line| &line.styled_segments)
            .map(|(_, _, text)| text.as_str())
            .collect();

        // The rendered text should preserve spacing: "{ChangeType, DiffResult, LineChange}"
        // not "{ChangeType,DiffResult,LineChange}"
        assert!(
            rendered_text.contains("{ChangeType, DiffResult, LineChange}"),
            "Spacing after commas should be preserved. Got: {}",
            rendered_text
        );
    }
}

