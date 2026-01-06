use crate::models::{Line, Mode};
use crate::text::wrap_text;
use crossterm::{
    event::{KeyCode, KeyEvent, KeyModifiers},
    terminal,
};
use std::io;

/// Handles key events in normal mode. Returns false to exit the application.
pub fn handle_normal_mode(
    key: KeyEvent,
    lines: &mut [Line],
    cursor_line: &mut usize,
    mode: &mut Mode,
    theme: &mut crate::theme::Theme,
    modified: &mut bool,
    annotation_scroll: &mut usize,
    scroll_offset: &mut usize,
) -> io::Result<bool> {
    match (key.code, key.modifiers) {
        (KeyCode::Char('x'), KeyModifiers::CONTROL) => return Ok(false),
        (KeyCode::Char('t'), KeyModifiers::CONTROL) => {
            *theme = match *theme {
                crate::theme::Theme::Dark => crate::theme::Theme::Light,
                crate::theme::Theme::Light => crate::theme::Theme::Dark,
            };
        }
        (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
            if lines[*cursor_line].annotation.is_some() {
                lines[*cursor_line].annotation = None;
                *modified = true;
            }
        }
        (KeyCode::Up, _) => {
            if *cursor_line > 0 {
                *cursor_line -= 1;
                *annotation_scroll = 0;
                adjust_scroll(*cursor_line, scroll_offset)?;
            }
        }
        (KeyCode::Down, _) => {
            if *cursor_line < lines.len() - 1 {
                *cursor_line += 1;
                *annotation_scroll = 0;
                adjust_scroll(*cursor_line, scroll_offset)?;
            }
        }
        (KeyCode::PageUp, _) => {
            let (_, height) = terminal::size()?;
            *cursor_line = cursor_line.saturating_sub((height - 6) as usize);
            *annotation_scroll = 0;
            adjust_scroll(*cursor_line, scroll_offset)?;
        }
        (KeyCode::PageDown, _) => {
            let (_, height) = terminal::size()?;
            *cursor_line = (*cursor_line + (height - 6) as usize).min(lines.len() - 1);
            *annotation_scroll = 0;
            adjust_scroll(*cursor_line, scroll_offset)?;
        }
        (KeyCode::Enter, _) => {
            let existing = lines[*cursor_line].annotation.clone().unwrap_or_default();
            *annotation_scroll = 0;
            *mode = Mode::Annotating { buffer: existing, cursor_pos: 0 };
        }
        _ => {}
    }
    Ok(true)
}

/// Handles key events in annotation mode.
pub fn handle_annotation_mode(
    key: KeyEvent,
    buffer: &mut String,
    cursor_pos: &mut usize,
    lines: &mut [Line],
    cursor_line: usize,
    mode: &mut Mode,
    modified: &mut bool,
    annotation_scroll: &mut usize,
) -> io::Result<()> {
    match key.code {
        KeyCode::Enter => {
            if buffer.is_empty() {
                lines[cursor_line].annotation = None;
            } else {
                lines[cursor_line].annotation = Some(buffer.clone());
            }
            *modified = true;
            *annotation_scroll = 0;
            *mode = Mode::Normal;
        }
        KeyCode::Esc => {
            *annotation_scroll = 0;
            *mode = Mode::Normal;
        }
        KeyCode::Up => {
            move_cursor_up(buffer, cursor_pos, annotation_scroll)?;
        }
        KeyCode::Down => {
            move_cursor_down(buffer, cursor_pos, annotation_scroll)?;
        }
        KeyCode::Char(c) => {
            buffer.insert(*cursor_pos, c);
            *cursor_pos += 1;
            adjust_annotation_scroll(buffer, *cursor_pos, annotation_scroll)?;
        }
        KeyCode::Backspace => {
            if *cursor_pos > 0 {
                *cursor_pos -= 1;
                buffer.remove(*cursor_pos);
                adjust_annotation_scroll(buffer, *cursor_pos, annotation_scroll)?;
            }
        }
        KeyCode::Left => {
            *cursor_pos = cursor_pos.saturating_sub(1);
            adjust_annotation_scroll(buffer, *cursor_pos, annotation_scroll)?;
        }
        KeyCode::Right => {
            *cursor_pos = (*cursor_pos + 1).min(buffer.len());
            adjust_annotation_scroll(buffer, *cursor_pos, annotation_scroll)?;
        }
        KeyCode::Home => {
            *cursor_pos = 0;
            adjust_annotation_scroll(buffer, *cursor_pos, annotation_scroll)?;
        }
        KeyCode::End => {
            *cursor_pos = buffer.len();
            adjust_annotation_scroll(buffer, *cursor_pos, annotation_scroll)?;
        }
        _ => {}
    }
    Ok(())
}

/// Handles key events in search mode.
pub fn handle_search_mode(
    key: KeyEvent,
    query: &mut String,
    cursor_pos: &mut usize,
    mode: &mut Mode,
    search_matches: &mut Vec<usize>,
    current_match: &mut Option<usize>,
    lines: &[Line],
    cursor_line: &mut usize,
    scroll_offset: &mut usize,
) -> io::Result<()> {
    match key.code {
        KeyCode::Enter => {
            if !search_matches.is_empty() {
                next_search_match(search_matches, current_match, cursor_line);
                adjust_scroll(*cursor_line, scroll_offset)?;
            }
        }
        KeyCode::Esc => {
            search_matches.clear();
            *current_match = None;
            *mode = Mode::Normal;
        }
        KeyCode::Char(c) => {
            query.insert(*cursor_pos, c);
            *cursor_pos += 1;
            perform_search(query, lines, search_matches, current_match, cursor_line);
            adjust_scroll(*cursor_line, scroll_offset)?;
        }
        KeyCode::Backspace => {
            if *cursor_pos > 0 {
                *cursor_pos -= 1;
                query.remove(*cursor_pos);
                perform_search(query, lines, search_matches, current_match, cursor_line);
                adjust_scroll(*cursor_line, scroll_offset)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn move_cursor_up(buffer: &str, cursor_pos: &mut usize, annotation_scroll: &mut usize) -> io::Result<()> {
    let (width, _) = terminal::size()?;
    let max_width = width as usize - 4;
    let wrapped = wrap_text(buffer, max_width);
    
    if wrapped.is_empty() || buffer.is_empty() {
        return Ok(());
    }
    
    let chars: Vec<char> = buffer.chars().collect();
    let actual_pos = (*cursor_pos).min(chars.len());
    
    let mut chars_so_far = 0;
    let mut current_line = 0;
    let mut current_col = 0;
    
    for (line_idx, wrapped_line) in wrapped.iter().enumerate() {
        let wrapped_chars = wrapped_line.chars().count();
        let next_chars = chars_so_far + wrapped_chars;
        
        if actual_pos <= next_chars {
            current_line = line_idx;
            current_col = actual_pos - chars_so_far;
            break;
        }
        
        chars_so_far = next_chars;
        if line_idx < wrapped.len() - 1 && next_chars < chars.len() {
            chars_so_far += 1;
        }
    }
    
    if current_line > 0 {
        let target_line = current_line - 1;
        let target_line_len = wrapped[target_line].chars().count();
        let target_col = current_col.min(target_line_len);
        
        let mut new_pos = 0;
        for i in 0..target_line {
            new_pos += wrapped[i].chars().count();
            if i < wrapped.len() - 1 && new_pos < chars.len() {
                new_pos += 1;
            }
        }
        new_pos += target_col;
        
        *cursor_pos = new_pos.min(buffer.len());
        
        if target_line < *annotation_scroll {
            *annotation_scroll = target_line;
        }
    }
    Ok(())
}

fn move_cursor_down(buffer: &str, cursor_pos: &mut usize, annotation_scroll: &mut usize) -> io::Result<()> {
    let (width, _) = terminal::size()?;
    let max_width = width as usize - 4;
    let wrapped = wrap_text(buffer, max_width);
    
    if wrapped.is_empty() || buffer.is_empty() {
        return Ok(());
    }
    
    let chars: Vec<char> = buffer.chars().collect();
    let actual_pos = (*cursor_pos).min(chars.len());
    
    let mut chars_so_far = 0;
    let mut current_line = 0;
    let mut current_col = 0;
    
    for (line_idx, wrapped_line) in wrapped.iter().enumerate() {
        let wrapped_chars = wrapped_line.chars().count();
        let next_chars = chars_so_far + wrapped_chars;
        
        if actual_pos <= next_chars {
            current_line = line_idx;
            current_col = actual_pos - chars_so_far;
            break;
        }
        
        chars_so_far = next_chars;
        if line_idx < wrapped.len() - 1 && next_chars < chars.len() {
            chars_so_far += 1;
        }
    }
    
    if current_line < wrapped.len() - 1 {
        let target_line = current_line + 1;
        let target_line_len = wrapped[target_line].chars().count();
        let target_col = current_col.min(target_line_len);
        
        let mut new_pos = 0;
        for i in 0..target_line {
            new_pos += wrapped[i].chars().count();
            if i < wrapped.len() - 1 && new_pos < chars.len() {
                new_pos += 1;
            }
        }
        new_pos += target_col;
        
        *cursor_pos = new_pos.min(buffer.len());
        
        if target_line >= *annotation_scroll + 2 {
            *annotation_scroll = target_line - 1;
        }
    }
    Ok(())
}

fn perform_search(
    query: &str,
    lines: &[Line],
    search_matches: &mut Vec<usize>,
    current_match: &mut Option<usize>,
    cursor_line: &mut usize,
) {
    search_matches.clear();
    *current_match = None;

    if query.is_empty() {
        return;
    }

    let query_lower = query.to_lowercase();
    for (i, line) in lines.iter().enumerate() {
        if line.content.to_lowercase().contains(&query_lower) {
            search_matches.push(i);
        }
    }

    if !search_matches.is_empty() {
        *current_match = Some(0);
        *cursor_line = search_matches[0];
    }
}

fn next_search_match(
    search_matches: &[usize],
    current_match: &mut Option<usize>,
    cursor_line: &mut usize,
) {
    if let Some(idx) = *current_match {
        if !search_matches.is_empty() {
            let next = (idx + 1) % search_matches.len();
            *current_match = Some(next);
            *cursor_line = search_matches[next];
        }
    }
}

fn adjust_scroll(cursor_line: usize, scroll_offset: &mut usize) -> io::Result<()> {
    let (_, height) = terminal::size().unwrap_or((80, 24));
    let content_height = (height - 6) as usize;

    if cursor_line < *scroll_offset {
        *scroll_offset = cursor_line;
    } else if cursor_line >= *scroll_offset + content_height {
        *scroll_offset = cursor_line - content_height + 1;
    }
    Ok(())
}

fn adjust_annotation_scroll(
    buffer: &str,
    cursor_pos: usize,
    annotation_scroll: &mut usize,
) -> io::Result<()> {
    let (width, _) = terminal::size().unwrap_or((80, 24));
    let max_width = width as usize - 4;
    let wrapped = wrap_text(buffer, max_width);
    
    if wrapped.is_empty() || buffer.is_empty() {
        *annotation_scroll = 0;
        return Ok(());
    }

    let chars: Vec<char> = buffer.chars().collect();
    let actual_pos = cursor_pos.min(chars.len());
    
    let mut chars_so_far = 0;
    let mut current_line = 0;
    
    for (line_idx, wrapped_line) in wrapped.iter().enumerate() {
        let wrapped_chars = wrapped_line.chars().count();
        let next_chars = chars_so_far + wrapped_chars;
        
        if actual_pos <= next_chars {
            current_line = line_idx;
            break;
        }
        
        chars_so_far = next_chars;
        if line_idx < wrapped.len() - 1 && next_chars < chars.len() {
            chars_so_far += 1;
        }
    }

    if current_line < *annotation_scroll {
        *annotation_scroll = current_line;
    } else if current_line >= *annotation_scroll + 2 {
        *annotation_scroll = current_line - 1;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adjust_annotation_scroll_basic() {
        let mut scroll = 0;
        // Text that will wrap at 10 chars (width 14 - 4)
        let text = "one two three four five six";
        // "one two " (8 chars)
        // "three four " (11 chars -> "three ")
        // "four five " (10 chars)
        // Expected wrap (approx):
        // ["one two", "three four", "five six"]
        
        // Line 0: "one two " (8 chars)
        // Line 1: "three four " (11 chars)
        // Line 2: "five six"
        
        // Force width to 14
        let width: u16 = 14;
        let max_width = width as usize - 4; // 10
        let _wrapped = wrap_text(text, max_width);
        
        // Let's check where the lines wrap
        // "one two" is 7 chars. Next word "three" (5 chars). 7 + 1 + 5 = 13 > 10.
        // So:
        // L0: "one two" (7)
        // L1: "three four" (10)
        // L2: "five six" (8)
        
        // Cursor at start (L0)
        adjust_annotation_scroll_with_width(text, 0, &mut scroll, width).unwrap();
        assert_eq!(scroll, 0);
        
        // Cursor at end of L1 (pos 7 + 1 + 10 = 18)
        adjust_annotation_scroll_with_width(text, 18, &mut scroll, width).unwrap();
        assert_eq!(scroll, 0); // Still visible (L0 and L1)
        
        // Cursor at start of L2 (pos 19)
        adjust_annotation_scroll_with_width(text, 19, &mut scroll, width).unwrap();
        assert_eq!(scroll, 1); // Should scroll down
    }

    #[test]
    fn test_adjust_annotation_scroll_empty() {
        let mut scroll = 5;
        adjust_annotation_scroll_with_width("", 0, &mut scroll, 80).unwrap();
        assert_eq!(scroll, 0);
    }

    // Helper for testing with fixed width
    fn adjust_annotation_scroll_with_width(
        buffer: &str,
        cursor_pos: usize,
        annotation_scroll: &mut usize,
        width: u16,
    ) -> io::Result<()> {
        let max_width = width as usize - 4;
        let wrapped = wrap_text(buffer, max_width);
        
        if wrapped.is_empty() || buffer.is_empty() {
            *annotation_scroll = 0;
            return Ok(());
        }

        let chars: Vec<char> = buffer.chars().collect();
        let actual_pos = cursor_pos.min(chars.len());
        
        let mut chars_so_far = 0;
        let mut current_line = 0;
        
        for (line_idx, wrapped_line) in wrapped.iter().enumerate() {
            let wrapped_chars = wrapped_line.chars().count();
            let next_chars = chars_so_far + wrapped_chars;
            
            if actual_pos <= next_chars {
                current_line = line_idx;
                break;
            }
            
            chars_so_far = next_chars;
            if line_idx < wrapped.len() - 1 && next_chars < chars.len() {
                chars_so_far += 1;
            }
        }

        if current_line < *annotation_scroll {
            *annotation_scroll = current_line;
        } else if current_line >= *annotation_scroll + 2 {
            *annotation_scroll = current_line - 1;
        }

        Ok(())
    }
}
