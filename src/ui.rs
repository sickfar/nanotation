use crate::models::{Line, Mode};
use crate::text::{wrap_text, wrap_styled_text};
use crate::theme::{ColorScheme, Theme};
use crate::highlighting::{SyntaxHighlighter, to_crossterm_color};
use syntect::highlighting::FontStyle;
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    queue,
    style::{Print, ResetColor, SetBackgroundColor, SetForegroundColor, SetAttribute, Attribute},
    terminal::{self},
};
use std::io::{self, Write};
use std::path::Path;
use unicode_width::UnicodeWidthStr;

/// Renders the editor UI to the terminal.
pub fn render(
    lines: &[Line],
    cursor_line: usize,
    scroll_offset: usize,
    mode: &Mode,
    file_path: &Option<String>,
    modified: bool,
    theme: Theme,
    search_matches: &[usize],
    current_match: Option<usize>,
    annotation_scroll: usize,
    highlighter: &SyntaxHighlighter,
) -> io::Result<()> {
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
        mode,
        annotation_scroll,
        &colors,
        width,
        annotation_start,
    )?;

    // Render status bar
    render_status_bar(
        &mut stdout,
        mode,
        file_path,
        modified,
        cursor_line,
        lines.len(),
        search_matches,
        current_match,
        &colors,
        width,
        height,
    )?;

    match mode {
        Mode::Help => render_help_overlay(&mut stdout, &colors, width, height)?,
        _ => {},
    }

    // Position and show cursor if in annotation edit mode
    let is_editing = matches!(mode, Mode::Annotating { .. });
    if is_editing {
        if let Mode::Annotating { buffer, cursor_pos } = mode {
            position_cursor(
                &mut stdout,
                buffer,
                *cursor_pos,
                annotation_scroll,
                annotation_start,
                width,
            )?;
        }
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
    mode: &Mode,
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

    // Get annotation content
    let (annotation_text, _cursor_pos, _is_editing) = match mode {
        Mode::Annotating { buffer, cursor_pos } => {
            let text = if buffer.is_empty() {
                "[Type annotation here...]".to_string()
            } else {
                buffer.clone()
            };
            (text, *cursor_pos, true)
        }
        _ => {
            let text = if let Some(ref annotation) = lines[cursor_line].annotation {
                annotation.clone()
            } else {
                "[No annotation - Press Enter to add]".to_string()
            };
            (text, 0, false)
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

fn render_status_bar(
    stdout: &mut impl Write,
    mode: &Mode,
    file_path: &Option<String>,
    modified: bool,
    cursor_line: usize,
    total_lines: usize,
    search_matches: &[usize],
    current_match: Option<usize>,
    colors: &ColorScheme,
    width: u16,
    height: u16,
) -> io::Result<()> {
    let status = match mode {
        Mode::Normal => {
            let modified_flag = if modified { " [Modified]" } else { "" };
            let file = file_path.as_deref().unwrap_or("[No Name]");
            format!(" {} | Line {}/{}{}  ^G Help  ^X Exit  ^O Save  ^W Search  ^T Theme  ^D Del  ^N/^P Jump  ^Z/^Y Undo/Redo",
                file, cursor_line + 1, total_lines, modified_flag)
        }
        Mode::Annotating { .. } => {
            " Enter: Save  Esc: Cancel  ←→: Move cursor  ↑↓: Navigate lines".to_string()
        }
        Mode::Search { query, .. } => {
            let matches = if !search_matches.is_empty() {
                format!(" ({}/{})", current_match.map(|i| i + 1).unwrap_or(0), search_matches.len())
            } else {
                String::new()
            };
            format!(" Search: {}█{}  Enter: Next  Esc: Cancel", query, matches)
        }
        Mode::QuitPrompt => {
            " Unsaved changes! Save before exiting? (y/n/Esc)".to_string()
        }
        Mode::Help => {
            " Help Mode - Press any key to return".to_string()
        }
    };

    queue!(
        stdout,
        MoveTo(0, height - 1),
        SetBackgroundColor(colors.status_bg),
        SetForegroundColor(colors.status_fg),
        Print(format!("{:width$}", status.chars().take(width as usize - 1).collect::<String>(), width = width as usize - 1)),
        ResetColor
    )?;

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
        " ^D         Delete Annotation",
        " Enter      Add / Edit Annotation",
        " ^W         Search",
        " ^T         Toggle Theme",
        " ^O         Save File",
        " ^X         Exit",
        " ^G         Toggle Help",
        "",
        " Arrow Keys Navigation",
        " PgUp/PgDn  Page Navigation",
        " Alt+Up/Dn  Page Navigation",
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
