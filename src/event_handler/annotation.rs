//! Annotation mode event handler.
//!
//! Handles keyboard events when editing an annotation on a line.

use crate::models::{Action, Line};
use crate::navigation::{
    adjust_annotation_scroll_pure, find_next_word_boundary, find_prev_word_boundary,
    move_cursor_down_in_wrapped, move_cursor_up_in_wrapped,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal;
use std::io;

/// Result of handling key events in Annotation editing state.
pub enum AnnotationModeResult {
    /// Continue editing (buffer/cursor_pos may have changed)
    Continue,
    /// User saved the annotation (Enter)
    Save(Action),
    /// User cancelled (Esc) - discard changes
    Cancel,
}

/// Handles key events in Annotating state.
/// Does NOT modify editor_state - returns a result that caller interprets.
pub fn handle_annotation_input(
    key: KeyEvent,
    buffer: &mut String,
    cursor_pos: &mut usize,
    lines: &[Line],
    cursor_line: usize,
    annotation_scroll: &mut usize,
) -> io::Result<AnnotationModeResult> {
    match key.code {
        KeyCode::Enter => {
            let old_text = lines[cursor_line].annotation.clone();
            let new_text = if buffer.is_empty() {
                None
            } else {
                Some(buffer.clone())
            };

            // Even if nothing changed, treat Enter as "save" (exits annotation mode)
            if old_text != new_text {
                *annotation_scroll = 0;
                return Ok(AnnotationModeResult::Save(Action::EditAnnotation {
                    line_index: cursor_line,
                    old_text,
                    new_text,
                }));
            } else {
                *annotation_scroll = 0;
                // No change, but still exit annotation mode
                return Ok(AnnotationModeResult::Cancel);
            }
        }
        KeyCode::Esc => {
            *annotation_scroll = 0;
            return Ok(AnnotationModeResult::Cancel);
        }
        KeyCode::Up => {
            move_cursor_up(buffer, cursor_pos, annotation_scroll)?;
        }
        KeyCode::Down => {
            move_cursor_down(buffer, cursor_pos, annotation_scroll)?;
        }
        KeyCode::Char(c) => {
            // Convert character index to byte index for string operations
            let byte_idx = buffer.chars().take(*cursor_pos).map(|c| c.len_utf8()).sum();
            buffer.insert(byte_idx, c);
            *cursor_pos += 1;
            adjust_annotation_scroll(buffer, *cursor_pos, annotation_scroll)?;
        }
        KeyCode::Backspace => {
            if *cursor_pos > 0 {
                *cursor_pos -= 1;
                // Convert character index to byte index for string operations
                let byte_idx = buffer.chars().take(*cursor_pos).map(|c| c.len_utf8()).sum();
                buffer.remove(byte_idx);
                adjust_annotation_scroll(buffer, *cursor_pos, annotation_scroll)?;
            }
        }
        KeyCode::Left if key.modifiers.contains(KeyModifiers::ALT) => {
            *cursor_pos = find_prev_word_boundary(buffer, *cursor_pos);
            adjust_annotation_scroll(buffer, *cursor_pos, annotation_scroll)?;
        }
        KeyCode::Right if key.modifiers.contains(KeyModifiers::ALT) => {
            *cursor_pos = find_next_word_boundary(buffer, *cursor_pos);
            adjust_annotation_scroll(buffer, *cursor_pos, annotation_scroll)?;
        }
        KeyCode::Left => {
            *cursor_pos = cursor_pos.saturating_sub(1);
            adjust_annotation_scroll(buffer, *cursor_pos, annotation_scroll)?;
        }
        KeyCode::Right => {
            // Use character count, not byte length
            *cursor_pos = (*cursor_pos + 1).min(buffer.chars().count());
            adjust_annotation_scroll(buffer, *cursor_pos, annotation_scroll)?;
        }
        KeyCode::Home => {
            *cursor_pos = 0;
            adjust_annotation_scroll(buffer, *cursor_pos, annotation_scroll)?;
        }
        KeyCode::End => {
            // Use character count, not byte length
            *cursor_pos = buffer.chars().count();
            adjust_annotation_scroll(buffer, *cursor_pos, annotation_scroll)?;
        }
        _ => {}
    }
    Ok(AnnotationModeResult::Continue)
}

fn move_cursor_up(
    buffer: &str,
    cursor_pos: &mut usize,
    annotation_scroll: &mut usize,
) -> io::Result<()> {
    let (width, _) = terminal::size()?;
    let max_width = width as usize - 4;

    // Use pure function from navigation module
    *cursor_pos = move_cursor_up_in_wrapped(buffer, *cursor_pos, max_width);

    // Adjust scroll to keep cursor visible
    adjust_annotation_scroll(buffer, *cursor_pos, annotation_scroll)?;

    Ok(())
}

fn move_cursor_down(
    buffer: &str,
    cursor_pos: &mut usize,
    annotation_scroll: &mut usize,
) -> io::Result<()> {
    let (width, _) = terminal::size()?;
    let max_width = width as usize - 4;

    // Use pure function from navigation module
    *cursor_pos = move_cursor_down_in_wrapped(buffer, *cursor_pos, max_width);

    // Adjust scroll to keep cursor visible
    adjust_annotation_scroll(buffer, *cursor_pos, annotation_scroll)?;

    Ok(())
}

fn adjust_annotation_scroll(
    buffer: &str,
    cursor_pos: usize,
    annotation_scroll: &mut usize,
) -> io::Result<()> {
    let (width, _) = terminal::size().unwrap_or((80, 24));
    let max_width = width as usize - 4;
    let visible_lines = 2; // Annotation area shows 2 lines

    // Use the pure function from navigation module
    *annotation_scroll = adjust_annotation_scroll_pure(
        buffer,
        cursor_pos,
        *annotation_scroll,
        max_width,
        visible_lines,
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::wrap_text;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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

    // ========================================================================
    // Annotation Scroll Tests
    // ========================================================================

    #[test]
    fn test_adjust_annotation_scroll_basic() {
        let mut scroll = 0;
        let text = "one two three four five six";
        let width: u16 = 14;

        adjust_annotation_scroll_with_width(text, 0, &mut scroll, width).unwrap();
        assert_eq!(scroll, 0);

        adjust_annotation_scroll_with_width(text, 18, &mut scroll, width).unwrap();
        assert_eq!(scroll, 0);

        adjust_annotation_scroll_with_width(text, 19, &mut scroll, width).unwrap();
        assert_eq!(scroll, 1);
    }

    #[test]
    fn test_adjust_annotation_scroll_empty() {
        let mut scroll = 5;
        adjust_annotation_scroll_with_width("", 0, &mut scroll, 80).unwrap();
        assert_eq!(scroll, 0);
    }

    // ========================================================================
    // Annotation Mode Tests
    // ========================================================================

    #[test]
    fn test_annotation_input_enter_saves() {
        let lines = vec![Line {
            content: "line1".to_string(),
            annotation: None,
        }];
        let mut buffer = "new annotation".to_string();
        let mut cursor_pos = buffer.len();
        let mut annotation_scroll = 0;

        let result = handle_annotation_input(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut buffer,
            &mut cursor_pos,
            &lines,
            0,
            &mut annotation_scroll,
        )
        .unwrap();

        assert!(matches!(result, AnnotationModeResult::Save(_)));
    }

    #[test]
    fn test_annotation_input_esc_cancels() {
        let lines = vec![Line {
            content: "line1".to_string(),
            annotation: None,
        }];
        let mut buffer = "new annotation".to_string();
        let mut cursor_pos = buffer.len();
        let mut annotation_scroll = 0;

        let result = handle_annotation_input(
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            &mut buffer,
            &mut cursor_pos,
            &lines,
            0,
            &mut annotation_scroll,
        )
        .unwrap();

        assert!(matches!(result, AnnotationModeResult::Cancel));
    }

    #[test]
    fn test_annotation_input_char_appends() {
        let lines = vec![Line {
            content: "line1".to_string(),
            annotation: None,
        }];
        let mut buffer = "test".to_string();
        let mut cursor_pos = buffer.len();
        let mut annotation_scroll = 0;

        let result = handle_annotation_input(
            KeyEvent::new(KeyCode::Char('!'), KeyModifiers::NONE),
            &mut buffer,
            &mut cursor_pos,
            &lines,
            0,
            &mut annotation_scroll,
        )
        .unwrap();

        assert!(matches!(result, AnnotationModeResult::Continue));
        assert_eq!(buffer, "test!");
        assert_eq!(cursor_pos, 5);
    }

    #[test]
    fn test_annotation_input_backspace_removes() {
        let lines = vec![Line {
            content: "line1".to_string(),
            annotation: None,
        }];
        let mut buffer = "test".to_string();
        let mut cursor_pos = buffer.len();
        let mut annotation_scroll = 0;

        let result = handle_annotation_input(
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
            &mut buffer,
            &mut cursor_pos,
            &lines,
            0,
            &mut annotation_scroll,
        )
        .unwrap();

        assert!(matches!(result, AnnotationModeResult::Continue));
        assert_eq!(buffer, "tes");
        assert_eq!(cursor_pos, 3);
    }

    #[test]
    fn test_annotation_input_cyrillic_char() {
        // Test inserting Cyrillic character (multi-byte UTF-8)
        let lines = vec![Line {
            content: "line1".to_string(),
            annotation: None,
        }];
        let mut buffer = "Hello".to_string();
        let mut cursor_pos = buffer.chars().count(); // 5 characters
        let mut annotation_scroll = 0;

        // Insert Russian 'в' (2 bytes in UTF-8)
        let result = handle_annotation_input(
            KeyEvent::new(KeyCode::Char('в'), KeyModifiers::NONE),
            &mut buffer,
            &mut cursor_pos,
            &lines,
            0,
            &mut annotation_scroll,
        )
        .unwrap();

        assert!(matches!(result, AnnotationModeResult::Continue));
        assert_eq!(buffer, "Helloв");
        assert_eq!(cursor_pos, 6); // Character count, not byte count
    }

    #[test]
    fn test_annotation_input_emoji() {
        // Test inserting emoji (4 bytes in UTF-8)
        let lines = vec![Line {
            content: "line1".to_string(),
            annotation: None,
        }];
        let mut buffer = "Test".to_string();
        let mut cursor_pos = buffer.chars().count(); // 4 characters
        let mut annotation_scroll = 0;

        // Insert emoji 🎉 (4 bytes)
        let result = handle_annotation_input(
            KeyEvent::new(KeyCode::Char('🎉'), KeyModifiers::NONE),
            &mut buffer,
            &mut cursor_pos,
            &lines,
            0,
            &mut annotation_scroll,
        )
        .unwrap();

        assert!(matches!(result, AnnotationModeResult::Continue));
        assert_eq!(buffer, "Test🎉");
        assert_eq!(cursor_pos, 5); // 5 characters total
    }

    #[test]
    fn test_annotation_input_backspace_cyrillic() {
        // Test backspace with Cyrillic character
        let lines = vec![Line {
            content: "line1".to_string(),
            annotation: None,
        }];
        let mut buffer = "Helloв".to_string(); // 'в' is 2 bytes
        let mut cursor_pos = buffer.chars().count(); // 6 characters
        let mut annotation_scroll = 0;

        let result = handle_annotation_input(
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
            &mut buffer,
            &mut cursor_pos,
            &lines,
            0,
            &mut annotation_scroll,
        )
        .unwrap();

        assert!(matches!(result, AnnotationModeResult::Continue));
        assert_eq!(buffer, "Hello");
        assert_eq!(cursor_pos, 5);
    }

    #[test]
    fn test_annotation_input_mixed_multibyte() {
        // Test inserting multiple multi-byte characters in sequence
        let lines = vec![Line {
            content: "line1".to_string(),
            annotation: None,
        }];
        let mut buffer = String::new();
        let mut cursor_pos = 0;
        let mut annotation_scroll = 0;

        // Insert Russian "Привет" character by character
        for c in "Привет".chars() {
            handle_annotation_input(
                KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE),
                &mut buffer,
                &mut cursor_pos,
                &lines,
                0,
                &mut annotation_scroll,
            )
            .unwrap();
        }

        assert_eq!(buffer, "Привет");
        assert_eq!(cursor_pos, 6); // 6 characters
        assert_eq!(buffer.len(), 12); // 12 bytes (each Cyrillic char is 2 bytes)
    }

    #[test]
    fn test_annotation_input_right_arrow_with_cyrillic() {
        // Test Right arrow key with Cyrillic text
        let lines = vec![Line {
            content: "line1".to_string(),
            annotation: None,
        }];
        let mut buffer = "Привет".to_string(); // 6 chars, 12 bytes
        let mut cursor_pos = 0;
        let mut annotation_scroll = 0;

        // Move to end
        let result = handle_annotation_input(
            KeyEvent::new(KeyCode::End, KeyModifiers::NONE),
            &mut buffer,
            &mut cursor_pos,
            &lines,
            0,
            &mut annotation_scroll,
        )
        .unwrap();

        assert!(matches!(result, AnnotationModeResult::Continue));
        assert_eq!(cursor_pos, 6); // Should be character count, not byte length

        // Move right should stay at end
        handle_annotation_input(
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            &mut buffer,
            &mut cursor_pos,
            &lines,
            0,
            &mut annotation_scroll,
        )
        .unwrap();

        assert_eq!(cursor_pos, 6); // Should still be at end
    }

    #[test]
    fn test_annotation_input_insert_middle_cyrillic() {
        // Test inserting character in middle of Cyrillic text
        let lines = vec![Line {
            content: "line1".to_string(),
            annotation: None,
        }];
        let mut buffer = "При".to_string(); // 3 Cyrillic chars
        let mut cursor_pos = 2; // After "Пр"
        let mut annotation_scroll = 0;

        // Insert 'и'
        let result = handle_annotation_input(
            KeyEvent::new(KeyCode::Char('и'), KeyModifiers::NONE),
            &mut buffer,
            &mut cursor_pos,
            &lines,
            0,
            &mut annotation_scroll,
        )
        .unwrap();

        assert!(matches!(result, AnnotationModeResult::Continue));
        assert_eq!(buffer, "Прии");
        assert_eq!(cursor_pos, 3);
    }

    // ========================================================================
    // Alt+Left/Alt+Right Word Navigation Tests
    // ========================================================================

    #[test]
    fn test_annotation_input_alt_right_basic() {
        let lines = vec![Line {
            content: "line1".to_string(),
            annotation: None,
        }];
        let mut buffer = "hello world foo".to_string();
        let mut cursor_pos = 0;
        let mut annotation_scroll = 0;

        // Alt+Right from start
        let result = handle_annotation_input(
            KeyEvent::new(KeyCode::Right, KeyModifiers::ALT),
            &mut buffer,
            &mut cursor_pos,
            &lines,
            0,
            &mut annotation_scroll,
        )
        .unwrap();

        assert!(matches!(result, AnnotationModeResult::Continue));
        assert_eq!(cursor_pos, 6); // Jump to "world"
    }

    #[test]
    fn test_annotation_input_alt_left_basic() {
        let lines = vec![Line {
            content: "line1".to_string(),
            annotation: None,
        }];
        let mut buffer = "hello world foo".to_string();
        let mut cursor_pos = 15; // At end
        let mut annotation_scroll = 0;

        // Alt+Left from end
        let result = handle_annotation_input(
            KeyEvent::new(KeyCode::Left, KeyModifiers::ALT),
            &mut buffer,
            &mut cursor_pos,
            &lines,
            0,
            &mut annotation_scroll,
        )
        .unwrap();

        assert!(matches!(result, AnnotationModeResult::Continue));
        assert_eq!(cursor_pos, 12); // Jump to "foo"
    }

    #[test]
    fn test_annotation_input_word_nav_cyrillic() {
        let lines = vec![Line {
            content: "line1".to_string(),
            annotation: None,
        }];
        let mut buffer = "Привет мир тест".to_string();
        let mut cursor_pos = 0;
        let mut annotation_scroll = 0;

        // Alt+Right through Cyrillic text
        handle_annotation_input(
            KeyEvent::new(KeyCode::Right, KeyModifiers::ALT),
            &mut buffer,
            &mut cursor_pos,
            &lines,
            0,
            &mut annotation_scroll,
        )
        .unwrap();

        assert_eq!(cursor_pos, 7); // After "Привет "

        // Continue to next word
        handle_annotation_input(
            KeyEvent::new(KeyCode::Right, KeyModifiers::ALT),
            &mut buffer,
            &mut cursor_pos,
            &lines,
            0,
            &mut annotation_scroll,
        )
        .unwrap();

        assert_eq!(cursor_pos, 11); // After "мир "

        // Alt+Left to go back
        handle_annotation_input(
            KeyEvent::new(KeyCode::Left, KeyModifiers::ALT),
            &mut buffer,
            &mut cursor_pos,
            &lines,
            0,
            &mut annotation_scroll,
        )
        .unwrap();

        assert_eq!(cursor_pos, 7); // Back to "мир"
    }

    #[test]
    fn test_annotation_input_word_nav_with_punctuation() {
        let lines = vec![Line {
            content: "line1".to_string(),
            annotation: None,
        }];
        let mut buffer = "TODO: fix bug".to_string();
        let mut cursor_pos = 0;
        let mut annotation_scroll = 0;

        // Alt+Right skips boundaries and jumps to next word
        handle_annotation_input(
            KeyEvent::new(KeyCode::Right, KeyModifiers::ALT),
            &mut buffer,
            &mut cursor_pos,
            &lines,
            0,
            &mut annotation_scroll,
        )
        .unwrap();

        assert_eq!(cursor_pos, 6); // Jump to "fix" (skip "TODO:" and space)

        handle_annotation_input(
            KeyEvent::new(KeyCode::Right, KeyModifiers::ALT),
            &mut buffer,
            &mut cursor_pos,
            &lines,
            0,
            &mut annotation_scroll,
        )
        .unwrap();

        assert_eq!(cursor_pos, 10); // Jump to "bug" (skip space)
    }

    #[test]
    fn test_annotation_input_word_nav_mixed_unicode() {
        let lines = vec![Line {
            content: "line1".to_string(),
            annotation: None,
        }];
        let mut buffer = "Fix функцию get_data() error".to_string();
        let mut cursor_pos = 0;
        let mut annotation_scroll = 0;

        // Alt+Right through mixed English/Cyrillic
        handle_annotation_input(
            KeyEvent::new(KeyCode::Right, KeyModifiers::ALT),
            &mut buffer,
            &mut cursor_pos,
            &lines,
            0,
            &mut annotation_scroll,
        )
        .unwrap();

        assert!(cursor_pos > 0); // Moved forward

        // Continue navigating
        let mut count = 0;
        while cursor_pos < buffer.chars().count() && count < 10 {
            handle_annotation_input(
                KeyEvent::new(KeyCode::Right, KeyModifiers::ALT),
                &mut buffer,
                &mut cursor_pos,
                &lines,
                0,
                &mut annotation_scroll,
            )
            .unwrap();
            count += 1;
        }

        // Should reach end without panic
        assert_eq!(cursor_pos, buffer.chars().count());
    }

    #[test]
    fn test_annotation_input_word_nav_emoji() {
        let lines = vec![Line {
            content: "line1".to_string(),
            annotation: None,
        }];
        let mut buffer = "Done 🎉 success".to_string();
        let mut cursor_pos = 0;
        let mut annotation_scroll = 0;

        // Alt+Right past emoji
        handle_annotation_input(
            KeyEvent::new(KeyCode::Right, KeyModifiers::ALT),
            &mut buffer,
            &mut cursor_pos,
            &lines,
            0,
            &mut annotation_scroll,
        )
        .unwrap();

        assert_eq!(cursor_pos, 5); // After "Done "

        handle_annotation_input(
            KeyEvent::new(KeyCode::Right, KeyModifiers::ALT),
            &mut buffer,
            &mut cursor_pos,
            &lines,
            0,
            &mut annotation_scroll,
        )
        .unwrap();

        assert_eq!(cursor_pos, 7); // After "🎉 "
    }

    #[test]
    fn test_annotation_input_word_nav_at_boundaries() {
        let lines = vec![Line {
            content: "line1".to_string(),
            annotation: None,
        }];
        let mut buffer = "hello".to_string();
        let mut cursor_pos = 0;
        let mut annotation_scroll = 0;

        // Alt+Left at start should stay at start
        handle_annotation_input(
            KeyEvent::new(KeyCode::Left, KeyModifiers::ALT),
            &mut buffer,
            &mut cursor_pos,
            &lines,
            0,
            &mut annotation_scroll,
        )
        .unwrap();

        assert_eq!(cursor_pos, 0);

        // Jump to end
        cursor_pos = buffer.chars().count();

        // Alt+Right at end should stay at end
        handle_annotation_input(
            KeyEvent::new(KeyCode::Right, KeyModifiers::ALT),
            &mut buffer,
            &mut cursor_pos,
            &lines,
            0,
            &mut annotation_scroll,
        )
        .unwrap();

        assert_eq!(cursor_pos, buffer.chars().count());
    }

    // ========================================================================
    // Edge Cases for Cursor Positions
    // ========================================================================

    #[test]
    fn test_annotation_cursor_position_empty_buffer() {
        let lines = vec![Line {
            content: "line1".to_string(),
            annotation: None,
        }];
        let mut buffer = String::new();
        let mut cursor_pos = 0;
        let mut annotation_scroll = 0;

        // Try to delete from empty buffer
        let result = handle_annotation_input(
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
            &mut buffer,
            &mut cursor_pos,
            &lines,
            0,
            &mut annotation_scroll,
        )
        .unwrap();

        assert!(matches!(result, AnnotationModeResult::Continue));
        assert_eq!(cursor_pos, 0);
        assert_eq!(buffer, "");
    }

    #[test]
    fn test_annotation_cursor_position_backspace_at_start() {
        let lines = vec![Line {
            content: "line1".to_string(),
            annotation: None,
        }];
        let mut buffer = "test".to_string();
        let mut cursor_pos = 0; // Cursor at start
        let mut annotation_scroll = 0;

        // Backspace at start should do nothing
        let result = handle_annotation_input(
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
            &mut buffer,
            &mut cursor_pos,
            &lines,
            0,
            &mut annotation_scroll,
        )
        .unwrap();

        assert!(matches!(result, AnnotationModeResult::Continue));
        assert_eq!(buffer, "test"); // Buffer unchanged
        assert_eq!(cursor_pos, 0); // Cursor stays at 0
    }

    #[test]
    fn test_annotation_cursor_movement_home_end() {
        let lines = vec![Line {
            content: "line1".to_string(),
            annotation: None,
        }];
        let mut buffer = "hello world".to_string();
        let mut cursor_pos = 5;
        let mut annotation_scroll = 0;

        // Press Home
        handle_annotation_input(
            KeyEvent::new(KeyCode::Home, KeyModifiers::NONE),
            &mut buffer,
            &mut cursor_pos,
            &lines,
            0,
            &mut annotation_scroll,
        )
        .unwrap();
        assert_eq!(cursor_pos, 0);

        // Press End
        handle_annotation_input(
            KeyEvent::new(KeyCode::End, KeyModifiers::NONE),
            &mut buffer,
            &mut cursor_pos,
            &lines,
            0,
            &mut annotation_scroll,
        )
        .unwrap();
        assert_eq!(cursor_pos, buffer.chars().count());
    }

    #[test]
    fn test_annotation_cursor_left_right() {
        let lines = vec![Line {
            content: "line1".to_string(),
            annotation: None,
        }];
        let mut buffer = "test".to_string();
        let mut cursor_pos = 2;
        let mut annotation_scroll = 0;

        // Press Left
        handle_annotation_input(
            KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
            &mut buffer,
            &mut cursor_pos,
            &lines,
            0,
            &mut annotation_scroll,
        )
        .unwrap();
        assert_eq!(cursor_pos, 1);

        // Press Right
        handle_annotation_input(
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            &mut buffer,
            &mut cursor_pos,
            &lines,
            0,
            &mut annotation_scroll,
        )
        .unwrap();
        assert_eq!(cursor_pos, 2);
    }
}
