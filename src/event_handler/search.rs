//! Search mode event handler.
//!
//! Handles keyboard events when searching for text in the document.

use crate::diff::adjust_diff_scroll;
use crate::models::{Line, ViewMode};
use crate::navigation::{adjust_normal_scroll, cycle_match, find_matches, CycleDirection};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal;
use std::io;

/// Result of handling key events in Searching state.
pub enum SearchModeResult {
    /// Continue searching (query may have changed)
    Continue,
    /// User exited search (Esc)
    Exit,
}

/// Handles key events in Searching state.
/// Does NOT modify editor_state - returns a result that caller interprets.
pub fn handle_search_input(
    key: KeyEvent,
    query: &mut String,
    cursor_pos: &mut usize,
    search_matches: &mut Vec<usize>,
    current_match: &mut Option<usize>,
    lines: &[Line],
    cursor_line: &mut usize,
    scroll_offset: &mut usize,
    view_mode: &ViewMode,
    content_height: usize,
) -> io::Result<SearchModeResult> {
    match (key.code, key.modifiers) {
        (KeyCode::Enter, KeyModifiers::SHIFT) => {
            // Shift+Enter: previous match
            if !search_matches.is_empty() {
                prev_search_match(search_matches, current_match, cursor_line);
                adjust_scroll_unified(*cursor_line, scroll_offset, lines, view_mode, content_height)?;
            }
        }
        (KeyCode::Enter, _) => {
            // Enter: next match
            if !search_matches.is_empty() {
                next_search_match(search_matches, current_match, cursor_line);
                adjust_scroll_unified(*cursor_line, scroll_offset, lines, view_mode, content_height)?;
            }
        }
        (KeyCode::Esc, _) => {
            search_matches.clear();
            *current_match = None;
            return Ok(SearchModeResult::Exit);
        }
        (KeyCode::Char(c), _) => {
            query.insert(*cursor_pos, c);
            *cursor_pos += 1;
            perform_search(query, lines, search_matches, current_match, cursor_line);
            adjust_scroll_unified(*cursor_line, scroll_offset, lines, view_mode, content_height)?;
        }
        (KeyCode::Backspace, _) => {
            if *cursor_pos > 0 {
                *cursor_pos -= 1;
                query.remove(*cursor_pos);
                perform_search(query, lines, search_matches, current_match, cursor_line);
                adjust_scroll_unified(*cursor_line, scroll_offset, lines, view_mode, content_height)?;
            }
        }
        _ => {}
    }
    Ok(SearchModeResult::Continue)
}

fn perform_search(
    query: &str,
    lines: &[Line],
    search_matches: &mut Vec<usize>,
    current_match: &mut Option<usize>,
    cursor_line: &mut usize,
) {
    // Use pure function from navigation module
    *search_matches = find_matches(query, lines);
    *current_match = None;

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
    if let Some((new_idx, line)) = cycle_match(search_matches, *current_match, CycleDirection::Next)
    {
        *current_match = Some(new_idx);
        *cursor_line = line;
    }
}

fn prev_search_match(
    search_matches: &[usize],
    current_match: &mut Option<usize>,
    cursor_line: &mut usize,
) {
    if let Some((new_idx, line)) =
        cycle_match(search_matches, *current_match, CycleDirection::Previous)
    {
        *current_match = Some(new_idx);
        *cursor_line = line;
    }
}

/// Unified scroll adjustment that works for both Normal and Diff view modes.
fn adjust_scroll_unified(
    cursor_line: usize,
    scroll_offset: &mut usize,
    lines: &[Line],
    view_mode: &ViewMode,
    content_height: usize,
) -> io::Result<()> {
    let (width, _) = terminal::size().unwrap_or((80, 24));

    match view_mode {
        ViewMode::Diff { diff_result } => {
            // Use diff-aware scroll adjustment
            *scroll_offset =
                adjust_diff_scroll(cursor_line, *scroll_offset, content_height, diff_result);
        }
        ViewMode::Normal => {
            // Use normal scroll adjustment
            *scroll_offset = adjust_normal_scroll(
                cursor_line,
                *scroll_offset,
                content_height,
                lines,
                width as usize,
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn test_search_input_esc_exits() {
        let lines = vec![Line {
            content: "test line".to_string(),
            annotation: None,
        }];
        let mut query = "test".to_string();
        let mut cursor_pos = query.len();
        let mut search_matches = vec![0];
        let mut current_match = Some(0);
        let mut cursor_line = 0;
        let mut scroll_offset = 0;
        let view_mode = ViewMode::Normal;

        let result = handle_search_input(
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            &mut query,
            &mut cursor_pos,
            &mut search_matches,
            &mut current_match,
            &lines,
            &mut cursor_line,
            &mut scroll_offset,
            &view_mode,
            50,
        )
        .unwrap();

        assert!(matches!(result, SearchModeResult::Exit));
        assert!(search_matches.is_empty());
        assert!(current_match.is_none());
    }

    #[test]
    fn test_search_input_char_updates_query() {
        let lines = vec![Line {
            content: "test line".to_string(),
            annotation: None,
        }];
        let mut query = "tes".to_string();
        let mut cursor_pos = query.len();
        let mut search_matches = vec![];
        let mut current_match = None;
        let mut cursor_line = 0;
        let mut scroll_offset = 0;
        let view_mode = ViewMode::Normal;

        let result = handle_search_input(
            KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE),
            &mut query,
            &mut cursor_pos,
            &mut search_matches,
            &mut current_match,
            &lines,
            &mut cursor_line,
            &mut scroll_offset,
            &view_mode,
            50,
        )
        .unwrap();

        assert!(matches!(result, SearchModeResult::Continue));
        assert_eq!(query, "test");
    }

    #[test]
    fn test_search_cursor_position_updates() {
        let lines = vec![
            Line {
                content: "first match here".to_string(),
                annotation: None,
            },
            Line {
                content: "no match".to_string(),
                annotation: None,
            },
            Line {
                content: "second match here".to_string(),
                annotation: None,
            },
        ];
        let mut query = "match".to_string();
        let mut cursor_pos = query.len();
        let mut search_matches = vec![0, 2];
        let mut current_match = Some(0);
        let mut cursor_line = 0;
        let mut scroll_offset = 0;
        let view_mode = ViewMode::Normal;

        // Press Enter to go to next match
        handle_search_input(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut query,
            &mut cursor_pos,
            &mut search_matches,
            &mut current_match,
            &lines,
            &mut cursor_line,
            &mut scroll_offset,
            &view_mode,
            50,
        )
        .unwrap();

        assert_eq!(current_match, Some(1));
        assert_eq!(cursor_line, 2); // Should move to line 2
    }
}
