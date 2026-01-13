use crate::highlighting::{to_crossterm_color, SyntaxHighlighter};
use crate::models::{EditorState, Line, ViewMode};
use crate::text::{wrap_styled_text, wrap_text};
use crate::theme::{ColorScheme, Theme};
use crate::ui_diff::render_diff_mode;
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
) -> io::Result<()> {
    // Check if we're in diff view mode
    if let ViewMode::Diff { diff_result } = view_mode {
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
        );
    }
    let (width, height) = terminal::size()?;
    // Reserve 5 lines at bottom: 4 for annotation area (border + 2 text lines + border) + 1 for status bar
    let content_height = (height - 5) as usize;
    let colors = theme.colors();

    let mut stdout = io::stdout();
    queue!(stdout, MoveTo(0, 0))?;

    let gutter_width = lines.len().to_string().len() + 2; // + padding
    let content_width = (width as usize).saturating_sub(gutter_width);

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
                MoveTo(0, screen_line as u16),
                SetBackgroundColor(colors.bg),
                SetForegroundColor(colors.status_fg),
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
            MoveTo(0, screen_line as u16),
            SetBackgroundColor(colors.bg),
            Print(format!("{:width$}", "", width = width as usize)),
            ResetColor
        )?;
        screen_line += 1;
    }

    // Render annotation area (4 lines: border + 2 text lines + border) above status bar
    let annotation_start = height - 5;

    render_annotation_area(
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
    render_status_bar(
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
        width,
        height,
        status_message,
        diff_available,
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
            width,
        )?;
    } else {
        queue!(stdout, Hide)?;
    }

    stdout.flush()?;
    Ok(())
}

fn render_annotation_area(
    stdout: &mut impl Write,
    lines: &[Line],
    cursor_line: usize,
    editor_state: &EditorState,
    annotation_scroll: usize,
    colors: &ColorScheme,
    width: u16,
    annotation_start: u16,
) -> io::Result<()> {
    // Top border of annotation area
    queue!(
        stdout,
        MoveTo(0, annotation_start),
        SetBackgroundColor(colors.annotation_window_bg),
        SetForegroundColor(colors.annotation_window_fg),
        Print(format!("╔{}╗", "═".repeat(width as usize - 2))),
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
            if let Some(ref annotation) = lines[cursor_line].annotation {
                annotation.clone()
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
            Print(format!("║ {:width$} ║", display_line, width = max_annotation_width)),
            ResetColor
        )?;
    }

    // Bottom border of annotation area
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

#[allow(clippy::too_many_arguments)]
fn render_status_bar(
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
            let view_indicator = if matches!(view_mode, ViewMode::Diff { .. }) {
                "DIFF | "
            } else {
                ""
            };

            // Build the left part: filename and line info
            let left_part = format!(
                " {}{}{} | Line {}/{}",
                view_indicator, filename, modified_flag, cursor_line + 1, total_lines
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

            // Continue with the rest of the shortcuts
            let shortcuts = " ^G Help  ^X Exit  ^O Save  ^W Search  ^T Theme  Del/Bksp Del  ^N/^P Jump";
            let current_len = left_part.len() + if diff_available { 10 } else { 0 }; // " " + " ^D Diff "
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

fn position_cursor(
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
    
    let cursor_x = 2 + cursor_col.min(display_line.chars().count());
    
    queue!(
        stdout,
        MoveTo(cursor_x as u16, cursor_screen_line),
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
    let box_height = 18;
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
