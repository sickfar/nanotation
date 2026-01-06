#![allow(clippy::too_many_arguments)]
use crate::models::{Line, Mode, Action};
use crate::text::wrap_text;
use crossterm::{
    event::{KeyCode, KeyEvent, KeyModifiers},
    terminal,
};
use std::io;

pub enum NormalModeResult {
    Continue,
    Exit,
    Undo,
    Redo,
    Action(Action),
}

/// Handles key events in normal mode.
pub fn handle_normal_mode(
    key: KeyEvent,
    lines: &mut [Line],
    cursor_line: &mut usize,
    mode: &mut Mode,
    theme: &mut crate::theme::Theme,
    modified: &mut bool,
    annotation_scroll: &mut usize,
    scroll_offset: &mut usize,
) -> io::Result<NormalModeResult> {
    match (key.code, key.modifiers) {
        (KeyCode::Char('x'), KeyModifiers::CONTROL) => {
            if *modified {
                *mode = Mode::QuitPrompt;
                return Ok(NormalModeResult::Continue); // Mode changed, just continue loop to handle prompt
            } else {
                return Ok(NormalModeResult::Exit);
            }
        }
        (KeyCode::Char('t'), KeyModifiers::CONTROL) => {
            *theme = match *theme {
                crate::theme::Theme::Dark => crate::theme::Theme::Light,
                crate::theme::Theme::Light => crate::theme::Theme::Dark,
            };
        }
        (KeyCode::Char('g'), KeyModifiers::CONTROL) => {
            *mode = Mode::Help;
        }
        (KeyCode::Char('z'), KeyModifiers::CONTROL) => {
            return Ok(NormalModeResult::Undo);
        }
        (KeyCode::Char('y'), KeyModifiers::CONTROL) => {
            return Ok(NormalModeResult::Redo);
        }
        (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
            if let Some(old_text) = &lines[*cursor_line].annotation {
                // Return Action instead of mutating directly
                return Ok(NormalModeResult::Action(Action::EditAnnotation {
                    line_index: *cursor_line,
                    old_text: Some(old_text.clone()),
                    new_text: None,
                }));
            }
        }
        (KeyCode::Char('n'), KeyModifiers::CONTROL) => {
            // Jump to next annotation
            for i in (*cursor_line + 1)..lines.len() {
                if lines[i].annotation.is_some() {
                    *cursor_line = i;
                    *annotation_scroll = 0;
                    adjust_scroll(*cursor_line, scroll_offset, lines)?;
                    break;
                }
            }
        }
        (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
            // Jump to previous annotation
            for i in (0..*cursor_line).rev() {
                if lines[i].annotation.is_some() {
                    *cursor_line = i;
                    *annotation_scroll = 0;
                    adjust_scroll(*cursor_line, scroll_offset, lines)?;
                    break;
                }
            }
        }
        (KeyCode::PageUp, _) | (KeyCode::Up, KeyModifiers::ALT) => {
            let (_, height) = terminal::size()?;
            *cursor_line = cursor_line.saturating_sub((height - 5) as usize);
            *annotation_scroll = 0;
            adjust_scroll(*cursor_line, scroll_offset, lines)?;
        }
        (KeyCode::PageDown, _) | (KeyCode::Down, KeyModifiers::ALT) => {
            let (_, height) = terminal::size()?;
            *cursor_line = (*cursor_line + (height - 5) as usize).min(lines.len() - 1);
            *annotation_scroll = 0;
            adjust_scroll(*cursor_line, scroll_offset, lines)?;
        }
        (KeyCode::Up, _) => {
            if *cursor_line > 0 {
                *cursor_line -= 1;
                *annotation_scroll = 0;
                adjust_scroll(*cursor_line, scroll_offset, lines)?;
            }
        }
        (KeyCode::Down, _) => {
            if *cursor_line < lines.len() - 1 {
                *cursor_line += 1;
                *annotation_scroll = 0;
                adjust_scroll(*cursor_line, scroll_offset, lines)?;
            }
        }
        (KeyCode::Enter, _) => {
            let existing = lines[*cursor_line].annotation.clone().unwrap_or_default();
            *annotation_scroll = 0;
            *mode = Mode::Annotating { buffer: existing, cursor_pos: 0 };
        }
        _ => {}
    }
    Ok(NormalModeResult::Continue)
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
) -> io::Result<Option<Action>> {
    match key.code {
        KeyCode::Enter => {
            let old_text = lines[cursor_line].annotation.clone();
            let new_text = if buffer.is_empty() {
                None
            } else {
                Some(buffer.clone())
            };

            // Avoid action if nothing changed
            if old_text != new_text {
                // lines[cursor_line].annotation = new_text.clone(); // Don't mutate here if using perform_action
                // But perform_action calls this? No, editor calls perform_action.
                // We should return the action, and let Editor apply it.
                // But for visual feedback we might want to apply it... 
                // Actually Editor.perform_action will modify lines. So we shouldn't modify it here.
                
                *modified = true; // Editor.perform_action sets this too, but maybe redundant if we return Action.
                *annotation_scroll = 0;
                *mode = Mode::Normal;
                
                return Ok(Some(Action::EditAnnotation {
                    line_index: cursor_line,
                    old_text,
                    new_text,
                }));
            } else {
                 *annotation_scroll = 0;
                 *mode = Mode::Normal;
            }
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
    Ok(None)
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
                adjust_scroll(*cursor_line, scroll_offset, lines)?;
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
            adjust_scroll(*cursor_line, scroll_offset, lines)?;
        }
        KeyCode::Backspace => {
            if *cursor_pos > 0 {
                *cursor_pos -= 1;
                query.remove(*cursor_pos);
                perform_search(query, lines, search_matches, current_match, cursor_line);
                adjust_scroll(*cursor_line, scroll_offset, lines)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub enum QuitPromptResult {
    SaveAndExit,
    Exit,
    Cancel,
    Continue,
}

pub fn handle_quit_prompt(key: KeyEvent) -> QuitPromptResult {
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') => QuitPromptResult::SaveAndExit,
        KeyCode::Char('n') | KeyCode::Char('N') => QuitPromptResult::Exit,
        KeyCode::Esc | KeyCode::Char('c') | KeyCode::Char('C') => QuitPromptResult::Cancel,
        _ => QuitPromptResult::Continue,
    }
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
    if search_matches.is_empty() {
        return;
    }
    if let Some(idx) = *current_match {
        let next = (idx + 1) % search_matches.len();
        *current_match = Some(next);
        *cursor_line = search_matches[next];
    }
}

fn adjust_scroll(cursor_line: usize, scroll_offset: &mut usize, lines: &[Line]) -> io::Result<()> {
    let (width, height) = terminal::size().unwrap_or((80, 24));
    let content_height = (height - 5) as usize;

    if cursor_line < *scroll_offset {
        *scroll_offset = cursor_line;
    }

    let mut visual_lines = 0;
    for i in *scroll_offset..=cursor_line {
        if i >= lines.len() { break; }
        let wrapped = wrap_text(&lines[i].content, width as usize);
        visual_lines += if wrapped.is_empty() { 1 } else { wrapped.len() };
    }

    while visual_lines > content_height {
        if *scroll_offset >= cursor_line {
            break;
        }
        let wrapped = wrap_text(&lines[*scroll_offset].content, width as usize);
        let count = if wrapped.is_empty() { 1 } else { wrapped.len() };
        
        *scroll_offset += 1;
        visual_lines = visual_lines.saturating_sub(count);
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
    use crate::text::wrap_text;
    use crate::models::{Action, Line, Mode};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn test_adjust_annotation_scroll_basic() {
        let mut scroll = 0;
        // Text that will wrap at 10 chars (width 14 - 4)
        let text = "one two three four five six";
        let width: u16 = 14;
        
        // Line 0: "one two " (8 chars)
        // Line 1: "three four " (11 chars -> "three ") -> "three four" is 10 chars.
        // "one two" (7)
        // "three four" (10)
        // "five six" (8)

        adjust_annotation_scroll_with_width(text, 0, &mut scroll, width).unwrap();
        assert_eq!(scroll, 0);
        
        // Cursor at end of L1 (pos 18)
        adjust_annotation_scroll_with_width(text, 18, &mut scroll, width).unwrap();
        assert_eq!(scroll, 0); 
        
        // Cursor at start of L2 (pos 19)
        adjust_annotation_scroll_with_width(text, 19, &mut scroll, width).unwrap();
        assert_eq!(scroll, 1); 
    }

    #[test]
    fn test_adjust_annotation_scroll_empty() {
        let mut scroll = 5;
        adjust_annotation_scroll_with_width("", 0, &mut scroll, 80).unwrap();
        assert_eq!(scroll, 0);
    }

    #[test]
    fn test_jump_to_annotation() {
        let mut lines = vec![
            Line { content: "0".to_string(), annotation: None },
            Line { content: "1".to_string(), annotation: Some("a1".to_string()) },
            Line { content: "2".to_string(), annotation: None },
            Line { content: "3".to_string(), annotation: Some("a2".to_string()) },
            Line { content: "4".to_string(), annotation: None },
        ];
        
        let mut cursor_line = 0;
        let mut scroll_offset = 0;
        let mut annotation_scroll = 0;
        let mut mode = Mode::Normal;
        let mut theme = crate::theme::Theme::Dark;
        let mut modified = false;

        // Jump Next (from 0 to 1)
        let _ = handle_normal_mode(
            KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL),
            &mut lines,
            &mut cursor_line,
            &mut mode,
            &mut theme,
            &mut modified,
            &mut annotation_scroll,
            &mut scroll_offset,
        ).unwrap();
        assert_eq!(cursor_line, 1);

        // Jump Next (from 1 to 3)
        let _ = handle_normal_mode(
            KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL),
            &mut lines,
            &mut cursor_line,
            &mut mode,
            &mut theme,
            &mut modified,
            &mut annotation_scroll,
            &mut scroll_offset,
        ).unwrap();
        assert_eq!(cursor_line, 3);

        // Jump Next (from 3 - no next)
        let _ = handle_normal_mode(
            KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL),
            &mut lines,
            &mut cursor_line,
            &mut mode,
            &mut theme,
            &mut modified,
            &mut annotation_scroll,
            &mut scroll_offset,
        ).unwrap();
        assert_eq!(cursor_line, 3);

        // Jump Prev (from 3 to 1)
        let _ = handle_normal_mode(
            KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL),
            &mut lines,
            &mut cursor_line,
            &mut mode,
            &mut theme,
            &mut modified,
            &mut annotation_scroll,
            &mut scroll_offset,
        ).unwrap();
        assert_eq!(cursor_line, 1);
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
