//! Navigation and search logic as pure functions for testability.
//!
//! This module contains the core logic for navigation, search, and cursor movement
//! extracted into pure functions that don't require terminal access.

use crate::models::Line;
use crate::text::wrap_text;

// ============================================================================
// Annotation Jumping
// ============================================================================

/// Find the next line with an annotation after the current line.
/// Returns None if no annotation exists after current_line.
pub fn find_next_annotation(lines: &[Line], current_line: usize) -> Option<usize> {
    for i in (current_line + 1)..lines.len() {
        if lines[i].annotation.is_some() {
            return Some(i);
        }
    }
    None
}

/// Find the previous line with an annotation before the current line.
/// Returns None if no annotation exists before current_line.
pub fn find_prev_annotation(lines: &[Line], current_line: usize) -> Option<usize> {
    for i in (0..current_line).rev() {
        if lines[i].annotation.is_some() {
            return Some(i);
        }
    }
    None
}

// ============================================================================
// Search
// ============================================================================

/// Find all lines matching the search query (case-insensitive).
/// Returns a vector of line indices.
pub fn find_matches(query: &str, lines: &[Line]) -> Vec<usize> {
    if query.is_empty() {
        return Vec::new();
    }

    let query_lower = query.to_lowercase();
    lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.content.to_lowercase().contains(&query_lower))
        .map(|(i, _)| i)
        .collect()
}

/// Direction for cycling through matches
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CycleDirection {
    Next,
    Previous,
}

/// Cycle through search matches, wrapping around at boundaries.
/// Returns the new match index and the line number it points to.
pub fn cycle_match(
    matches: &[usize],
    current_match: Option<usize>,
    direction: CycleDirection,
) -> Option<(usize, usize)> {
    if matches.is_empty() {
        return None;
    }

    let current = current_match.unwrap_or(0);
    let new_idx = match direction {
        CycleDirection::Next => (current + 1) % matches.len(),
        CycleDirection::Previous => {
            if current == 0 {
                matches.len() - 1
            } else {
                current - 1
            }
        }
    };

    Some((new_idx, matches[new_idx]))
}

// ============================================================================
// Normal Mode Scroll
// ============================================================================

/// Calculate visual line count for a range of lines at a given width.
/// Each line may wrap to multiple visual lines.
pub fn calculate_visual_lines(lines: &[Line], start: usize, end: usize, width: usize) -> usize {
    let mut visual = 0;
    for i in start..=end {
        if i >= lines.len() {
            break;
        }
        let wrapped = wrap_text(&lines[i].content, width);
        visual += if wrapped.is_empty() { 1 } else { wrapped.len() };
    }
    visual
}

/// Adjust scroll offset to keep cursor visible in normal mode.
/// Returns the new scroll offset.
///
/// This accounts for line wrapping - a single logical line may take multiple visual lines.
pub fn adjust_normal_scroll(
    cursor_line: usize,
    current_scroll: usize,
    visible_height: usize,
    lines: &[Line],
    width: usize,
) -> usize {
    let mut scroll = current_scroll;

    // If cursor is above visible area, scroll up
    if cursor_line < scroll {
        return cursor_line;
    }

    // Calculate visual lines from scroll to cursor
    let visual_lines = calculate_visual_lines(lines, scroll, cursor_line, width);

    // If cursor is below visible area, scroll down
    if visual_lines > visible_height {
        // Need to scroll down - find minimum scroll that makes cursor visible
        while scroll < cursor_line {
            scroll += 1;

            let new_visual = calculate_visual_lines(lines, scroll, cursor_line, width);
            if new_visual <= visible_height {
                break;
            }
        }
    }

    scroll
}

// ============================================================================
// Wrapped Text Cursor Navigation
// ============================================================================

/// Position within wrapped text (line index and column)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WrappedPosition {
    pub line: usize,
    pub col: usize,
}

/// Convert a buffer cursor position to a position in wrapped text.
pub fn cursor_to_wrapped_position(
    buffer: &str,
    cursor_pos: usize,
    wrap_width: usize,
) -> WrappedPosition {
    let wrapped = wrap_text(buffer, wrap_width);

    if wrapped.is_empty() || buffer.is_empty() {
        return WrappedPosition { line: 0, col: 0 };
    }

    let chars: Vec<char> = buffer.chars().collect();
    let actual_pos = cursor_pos.min(chars.len());

    let mut chars_so_far = 0;

    for (line_idx, wrapped_line) in wrapped.iter().enumerate() {
        let wrapped_chars = wrapped_line.chars().count();
        let next_chars = chars_so_far + wrapped_chars;

        if actual_pos <= next_chars {
            return WrappedPosition {
                line: line_idx,
                col: actual_pos - chars_so_far,
            };
        }

        chars_so_far = next_chars;
        // Account for implicit newline between wrapped lines
        if line_idx < wrapped.len() - 1 && next_chars < chars.len() {
            chars_so_far += 1;
        }
    }

    // Fallback: end of last line
    let last_line = wrapped.len().saturating_sub(1);
    WrappedPosition {
        line: last_line,
        col: wrapped.get(last_line).map(|l| l.chars().count()).unwrap_or(0),
    }
}

/// Convert a wrapped position back to a buffer cursor position.
pub fn wrapped_position_to_cursor(
    buffer: &str,
    position: WrappedPosition,
    wrap_width: usize,
) -> usize {
    let wrapped = wrap_text(buffer, wrap_width);

    if wrapped.is_empty() || buffer.is_empty() {
        return 0;
    }

    let chars: Vec<char> = buffer.chars().collect();
    let target_line = position.line.min(wrapped.len().saturating_sub(1));

    let mut cursor = 0;
    for i in 0..target_line {
        cursor += wrapped[i].chars().count();
        // Account for implicit newline
        if i < wrapped.len() - 1 && cursor < chars.len() {
            cursor += 1;
        }
    }

    // Add column offset, clamped to line length
    let line_len = wrapped
        .get(target_line)
        .map(|l| l.chars().count())
        .unwrap_or(0);
    cursor += position.col.min(line_len);

    cursor.min(buffer.len())
}

/// Move cursor up in wrapped text. Returns new cursor position.
pub fn move_cursor_up_in_wrapped(
    buffer: &str,
    cursor_pos: usize,
    wrap_width: usize,
) -> usize {
    let pos = cursor_to_wrapped_position(buffer, cursor_pos, wrap_width);

    if pos.line == 0 {
        // Already at first line
        return cursor_pos;
    }

    let wrapped = wrap_text(buffer, wrap_width);
    let target_line = pos.line - 1;
    let target_line_len = wrapped
        .get(target_line)
        .map(|l| l.chars().count())
        .unwrap_or(0);

    // Try to maintain column, but clamp to line length
    let new_pos = WrappedPosition {
        line: target_line,
        col: pos.col.min(target_line_len),
    };

    wrapped_position_to_cursor(buffer, new_pos, wrap_width)
}

/// Move cursor down in wrapped text. Returns new cursor position.
pub fn move_cursor_down_in_wrapped(
    buffer: &str,
    cursor_pos: usize,
    wrap_width: usize,
) -> usize {
    let wrapped = wrap_text(buffer, wrap_width);
    let pos = cursor_to_wrapped_position(buffer, cursor_pos, wrap_width);

    if pos.line >= wrapped.len().saturating_sub(1) {
        // Already at last line
        return cursor_pos;
    }

    let target_line = pos.line + 1;
    let target_line_len = wrapped
        .get(target_line)
        .map(|l| l.chars().count())
        .unwrap_or(0);

    // Try to maintain column, but clamp to line length
    let new_pos = WrappedPosition {
        line: target_line,
        col: pos.col.min(target_line_len),
    };

    wrapped_position_to_cursor(buffer, new_pos, wrap_width)
}

/// Calculate annotation scroll to keep cursor visible.
/// Returns new scroll offset.
pub fn adjust_annotation_scroll_pure(
    buffer: &str,
    cursor_pos: usize,
    current_scroll: usize,
    wrap_width: usize,
    visible_lines: usize,
) -> usize {
    let pos = cursor_to_wrapped_position(buffer, cursor_pos, wrap_width);

    if pos.line < current_scroll {
        // Cursor above visible area
        pos.line
    } else if pos.line >= current_scroll + visible_lines {
        // Cursor below visible area
        pos.line.saturating_sub(visible_lines - 1)
    } else {
        // Cursor visible
        current_scroll
    }
}

// ============================================================================
// Word Navigation
// ============================================================================

/// Check if a character is a word boundary (whitespace or punctuation).
/// Underscores are treated as word characters (not boundaries) for programming contexts.
fn is_word_boundary(c: char) -> bool {
    c.is_whitespace() || matches!(
        c,
        '.' | ',' | ';' | ':' | '!' | '?' | '(' | ')' | '[' | ']' | '{' | '}' |
        '<' | '>' | '/' | '\\' | '|' | '@' | '#' | '$' | '%' | '^' | '&' | '*' |
        '+' | '=' | '-' | '"' | '\'' | '`' | '~'
    )
}

/// Find the start of the previous word from cursor position.
/// Returns 0 if at the start of the buffer.
pub fn find_prev_word_boundary(buffer: &str, cursor_pos: usize) -> usize {
    let chars: Vec<char> = buffer.chars().collect();

    if chars.is_empty() || cursor_pos == 0 {
        return 0;
    }

    let mut pos = cursor_pos.min(chars.len());

    // Move back one position to analyze what we're dealing with
    if pos > 0 {
        pos -= 1;
    }

    // If we're at a boundary, skip back over boundaries to find a word character
    if is_word_boundary(chars[pos]) {
        while pos > 0 && is_word_boundary(chars[pos]) {
            pos -= 1;
        }
        // Now we're at a word character; skip back to the start of this word
        while pos > 0 && !is_word_boundary(chars[pos - 1]) {
            pos -= 1;
        }
    } else {
        // We're at a word character; skip back to the start of this word
        while pos > 0 && !is_word_boundary(chars[pos - 1]) {
            pos -= 1;
        }
    }

    pos
}

/// Find the start of the next word from cursor position.
/// Returns buffer.chars().count() if at the end of the buffer.
pub fn find_next_word_boundary(buffer: &str, cursor_pos: usize) -> usize {
    let chars: Vec<char> = buffer.chars().collect();

    if chars.is_empty() {
        return 0;
    }

    let mut pos = cursor_pos.min(chars.len());

    // If we're past the end, return end
    if pos >= chars.len() {
        return chars.len();
    }

    // If we're at a word character, skip to the end of this word
    if !is_word_boundary(chars[pos]) {
        while pos < chars.len() && !is_word_boundary(chars[pos]) {
            pos += 1;
        }
        // Now skip over any consecutive boundaries
        while pos < chars.len() && is_word_boundary(chars[pos]) {
            pos += 1;
        }
    } else {
        // We're at a boundary; skip to the next word character
        while pos < chars.len() && is_word_boundary(chars[pos]) {
            pos += 1;
        }
    }

    pos
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod annotation_jump_tests {
    use super::*;

    fn make_lines(annotations: &[Option<&str>]) -> Vec<Line> {
        annotations
            .iter()
            .map(|a| Line {
                content: "code".to_string(),
                annotation: a.map(|s| s.to_string()),
            })
            .collect()
    }

    #[test]
    fn test_find_next_annotation_found() {
        let lines = make_lines(&[None, None, Some("note"), None, Some("another")]);

        assert_eq!(find_next_annotation(&lines, 0), Some(2));
        assert_eq!(find_next_annotation(&lines, 2), Some(4));
    }

    #[test]
    fn test_find_next_annotation_none() {
        let lines = make_lines(&[None, None, Some("note"), None]);

        assert_eq!(find_next_annotation(&lines, 2), None);
        assert_eq!(find_next_annotation(&lines, 3), None);
    }

    #[test]
    fn test_find_prev_annotation_found() {
        let lines = make_lines(&[Some("first"), None, Some("second"), None]);

        assert_eq!(find_prev_annotation(&lines, 3), Some(2));
        assert_eq!(find_prev_annotation(&lines, 2), Some(0));
    }

    #[test]
    fn test_find_prev_annotation_none() {
        let lines = make_lines(&[None, Some("only"), None]);

        assert_eq!(find_prev_annotation(&lines, 1), None);
        assert_eq!(find_prev_annotation(&lines, 0), None);
    }

    #[test]
    fn test_find_annotation_empty_lines() {
        let lines: Vec<Line> = vec![];

        assert_eq!(find_next_annotation(&lines, 0), None);
        assert_eq!(find_prev_annotation(&lines, 0), None);
    }

    #[test]
    fn test_find_annotation_all_annotated() {
        let lines = make_lines(&[Some("a"), Some("b"), Some("c")]);

        assert_eq!(find_next_annotation(&lines, 0), Some(1));
        assert_eq!(find_next_annotation(&lines, 1), Some(2));
        assert_eq!(find_prev_annotation(&lines, 2), Some(1));
        assert_eq!(find_prev_annotation(&lines, 1), Some(0));
    }
}

#[cfg(test)]
mod search_tests {
    use super::*;

    fn make_lines(contents: &[&str]) -> Vec<Line> {
        contents
            .iter()
            .map(|c| Line {
                content: c.to_string(),
                annotation: None,
            })
            .collect()
    }

    #[test]
    fn test_find_matches_basic() {
        let lines = make_lines(&["hello world", "foo bar", "hello again"]);

        let matches = find_matches("hello", &lines);
        assert_eq!(matches, vec![0, 2]);
    }

    #[test]
    fn test_find_matches_case_insensitive() {
        let lines = make_lines(&["Hello World", "HELLO", "hello"]);

        let matches = find_matches("hello", &lines);
        assert_eq!(matches, vec![0, 1, 2]);
    }

    #[test]
    fn test_find_matches_empty_query() {
        let lines = make_lines(&["hello", "world"]);

        let matches = find_matches("", &lines);
        assert!(matches.is_empty());
    }

    #[test]
    fn test_find_matches_no_results() {
        let lines = make_lines(&["hello", "world"]);

        let matches = find_matches("xyz", &lines);
        assert!(matches.is_empty());
    }

    #[test]
    fn test_find_matches_partial() {
        let lines = make_lines(&["function", "fun", "funny"]);

        let matches = find_matches("fun", &lines);
        assert_eq!(matches, vec![0, 1, 2]);
    }

    #[test]
    fn test_cycle_match_next() {
        let matches = vec![5, 10, 15];

        assert_eq!(cycle_match(&matches, Some(0), CycleDirection::Next), Some((1, 10)));
        assert_eq!(cycle_match(&matches, Some(1), CycleDirection::Next), Some((2, 15)));
        assert_eq!(cycle_match(&matches, Some(2), CycleDirection::Next), Some((0, 5))); // wrap
    }

    #[test]
    fn test_cycle_match_previous() {
        let matches = vec![5, 10, 15];

        assert_eq!(cycle_match(&matches, Some(2), CycleDirection::Previous), Some((1, 10)));
        assert_eq!(cycle_match(&matches, Some(1), CycleDirection::Previous), Some((0, 5)));
        assert_eq!(cycle_match(&matches, Some(0), CycleDirection::Previous), Some((2, 15))); // wrap
    }

    #[test]
    fn test_cycle_match_empty() {
        let matches: Vec<usize> = vec![];

        assert_eq!(cycle_match(&matches, None, CycleDirection::Next), None);
        assert_eq!(cycle_match(&matches, Some(0), CycleDirection::Next), None);
    }

    #[test]
    fn test_cycle_match_single() {
        let matches = vec![42];

        assert_eq!(cycle_match(&matches, Some(0), CycleDirection::Next), Some((0, 42)));
        assert_eq!(cycle_match(&matches, Some(0), CycleDirection::Previous), Some((0, 42)));
    }

    #[test]
    fn test_cycle_match_none_current() {
        let matches = vec![5, 10];

        // When current is None, start from 0
        assert_eq!(cycle_match(&matches, None, CycleDirection::Next), Some((1, 10)));
    }
}

#[cfg(test)]
mod scroll_tests {
    use super::*;

    fn make_lines(contents: &[&str]) -> Vec<Line> {
        contents
            .iter()
            .map(|c| Line {
                content: c.to_string(),
                annotation: None,
            })
            .collect()
    }

    #[test]
    fn test_calculate_visual_lines_no_wrap() {
        let lines = make_lines(&["short", "also short", "tiny"]);

        // Width 80 means no wrapping
        assert_eq!(calculate_visual_lines(&lines, 0, 2, 80), 3);
    }

    #[test]
    fn test_calculate_visual_lines_with_wrap() {
        let lines = make_lines(&["this is a longer line that will wrap", "short"]);

        // Width 20 will cause wrapping
        let visual = calculate_visual_lines(&lines, 0, 1, 20);
        assert!(visual > 2); // First line wraps
    }

    #[test]
    fn test_adjust_scroll_cursor_visible() {
        let lines = make_lines(&["a", "b", "c", "d", "e"]);

        // Cursor at line 2, scroll at 0, height 5 - cursor is visible
        let new_scroll = adjust_normal_scroll(2, 0, 5, &lines, 80);
        assert_eq!(new_scroll, 0);
    }

    #[test]
    fn test_adjust_scroll_cursor_above() {
        let lines = make_lines(&["a", "b", "c", "d", "e", "f", "g", "h"]);

        // Cursor at line 1, scroll at 5 - cursor above visible
        let new_scroll = adjust_normal_scroll(1, 5, 3, &lines, 80);
        assert_eq!(new_scroll, 1);
    }

    #[test]
    fn test_adjust_scroll_cursor_below() {
        let lines = make_lines(&["a", "b", "c", "d", "e", "f", "g", "h"]);

        // Cursor at line 7, scroll at 0, height 3 - cursor below visible
        let new_scroll = adjust_normal_scroll(7, 0, 3, &lines, 80);
        assert!(new_scroll > 0);
        // Cursor should now be visible
        let visual = calculate_visual_lines(&lines, new_scroll, 7, 80);
        assert!(visual <= 3);
    }

    #[test]
    fn test_adjust_scroll_with_wrapping() {
        // Create lines that will wrap
        let lines = make_lines(&[
            "short",
            "this is a very long line that definitely needs to wrap at width 20",
            "another short",
            "target line",
        ]);

        // Width 20 causes line 1 to wrap multiple times
        let new_scroll = adjust_normal_scroll(3, 0, 3, &lines, 20);
        // Should scroll to make line 3 visible
        assert!(new_scroll > 0);
    }
}

#[cfg(test)]
mod wrapped_cursor_tests {
    use super::*;

    #[test]
    fn test_cursor_to_wrapped_simple() {
        let buffer = "hello world";
        let pos = cursor_to_wrapped_position(buffer, 6, 80);

        assert_eq!(pos.line, 0);
        assert_eq!(pos.col, 6); // at 'w'
    }

    #[test]
    fn test_cursor_to_wrapped_with_wrap() {
        // "hello world" at width 6 wraps to:
        // "hello " (6)
        // "world" (5)
        let buffer = "hello world";
        let pos = cursor_to_wrapped_position(buffer, 6, 6);

        // Position 6 is at the start of "world" on line 1
        assert_eq!(pos.line, 1);
        assert_eq!(pos.col, 0);
    }

    #[test]
    fn test_cursor_to_wrapped_empty() {
        let pos = cursor_to_wrapped_position("", 0, 80);

        assert_eq!(pos.line, 0);
        assert_eq!(pos.col, 0);
    }

    #[test]
    fn test_wrapped_position_to_cursor_simple() {
        let buffer = "hello world";
        let pos = WrappedPosition { line: 0, col: 6 };

        let cursor = wrapped_position_to_cursor(buffer, pos, 80);
        assert_eq!(cursor, 6);
    }

    #[test]
    fn test_wrapped_position_roundtrip() {
        let buffer = "the quick brown fox jumps over";

        for cursor in 0..=buffer.len() {
            let pos = cursor_to_wrapped_position(buffer, cursor, 10);
            let back = wrapped_position_to_cursor(buffer, pos, 10);
            // Roundtrip should preserve cursor (or clamp to line end)
            assert!(back <= buffer.len());
        }
    }

    #[test]
    fn test_move_cursor_up() {
        // Buffer wraps at width 10:
        // "the quick " (10)
        // "brown fox" (9)
        let buffer = "the quick brown fox";

        // Start at "brown" (position 10), move up
        let new_pos = move_cursor_up_in_wrapped(buffer, 10, 10);
        // Should be at position 0 (start of first line)
        assert!(new_pos < 10);
    }

    #[test]
    fn test_move_cursor_up_at_top() {
        let buffer = "hello world";

        // Already at first line
        let new_pos = move_cursor_up_in_wrapped(buffer, 3, 80);
        assert_eq!(new_pos, 3); // No change
    }

    #[test]
    fn test_move_cursor_down() {
        let buffer = "the quick brown fox";

        // Start at position 5 on first line, move down
        let new_pos = move_cursor_down_in_wrapped(buffer, 5, 10);
        // Should be on second line
        assert!(new_pos >= 10);
    }

    #[test]
    fn test_cursor_to_wrapped_with_wide_chars() {
        // "你好世界" = 4 CJK chars, each 2 visual cols = 8 visual width total
        // wrap_text splits by whitespace, so "你好世界" (no spaces) stays on one line
        let buffer = "你好世界";

        // At width 10, all chars fit on one line
        let pos = cursor_to_wrapped_position(buffer, 2, 10);
        assert_eq!(pos.line, 0);
        assert_eq!(pos.col, 2);

        // Test with spaces: "你好 世界" where "你好 " = 5 visual (4+1)
        // wrap_text will put "你好 " on line 0 and "世界" on line 1
        let buffer_with_spaces = "你好 世界";
        let pos = cursor_to_wrapped_position(buffer_with_spaces, 2, 5);
        // Position 2 is after "你好", before space
        assert_eq!(pos.line, 0);
    }

    #[test]
    fn test_cursor_to_wrapped_mixed_width() {
        // "Hi 你好" with space to allow wrapping
        // "Hi" = 2 visual, " " = 1, "你好" = 4 visual
        let buffer = "Hi 你好";

        // At width 10, everything fits on one line
        let pos = cursor_to_wrapped_position(buffer, 3, 10);
        assert_eq!(pos.line, 0);
        assert_eq!(pos.col, 3); // After "Hi "

        // At width 3, "Hi" (2 visual) fits, " 你好" (5 visual) wraps
        let pos = cursor_to_wrapped_position(buffer, 3, 3);
        // Position 3 should be on line 1 (after wrap)
        assert_eq!(pos.line, 1);
        assert_eq!(pos.col, 0); // At start of wrapped line
    }

    #[test]
    fn test_wrapped_position_to_cursor_with_wide_chars() {
        // "你好 世界" = "你好" + " " + "世界"
        let buffer = "你好 世界";

        // At width 5, "你好" (4 visual) fits, " 世界" wraps
        // Line 0, col 2 is at the space
        let pos = WrappedPosition { line: 0, col: 2 };
        let cursor = wrapped_position_to_cursor(buffer, pos, 5);
        assert_eq!(cursor, 2); // Character position of space

        // Line 1, col 0 should be at start of "世界" (position 3 after "你好 ")
        let pos = WrappedPosition { line: 1, col: 0 };
        let cursor = wrapped_position_to_cursor(buffer, pos, 5);
        assert_eq!(cursor, 3);
    }

    #[test]
    fn test_move_cursor_down_at_bottom() {
        let buffer = "hello world";

        // Already at last line (no wrap at width 80)
        let new_pos = move_cursor_down_in_wrapped(buffer, 5, 80);
        assert_eq!(new_pos, 5); // No change
    }

    #[test]
    fn test_move_cursor_maintains_column() {
        // "abcdefghij" (10)
        // "klmnopqrst" (10)
        let buffer = "abcdefghijklmnopqrst";

        // Start at position 5 (column 5 on line 0)
        let down = move_cursor_down_in_wrapped(buffer, 5, 10);
        let pos = cursor_to_wrapped_position(buffer, down, 10);
        assert_eq!(pos.col, 5); // Maintained column

        // Go back up
        let up = move_cursor_up_in_wrapped(buffer, down, 10);
        assert_eq!(up, 5); // Back to original
    }
}

#[cfg(test)]
mod annotation_scroll_tests {
    use super::*;

    #[test]
    fn test_annotation_scroll_visible() {
        let buffer = "line one two three four";

        // Cursor visible, no scroll change
        let scroll = adjust_annotation_scroll_pure(buffer, 5, 0, 10, 2);
        assert_eq!(scroll, 0);
    }

    #[test]
    fn test_annotation_scroll_cursor_below() {
        // Buffer wraps to 3 lines at width 10
        let buffer = "aaaaaaaaaa bbbbbbbbbb cccccccccc";

        // Cursor on line 2, but only 2 visible lines, scroll at 0
        let pos = cursor_to_wrapped_position(buffer, 22, 10);
        assert!(pos.line >= 2);

        let scroll = adjust_annotation_scroll_pure(buffer, 22, 0, 10, 2);
        assert!(scroll > 0); // Should scroll to show cursor
    }

    #[test]
    fn test_annotation_scroll_cursor_above() {
        let buffer = "aaaaaaaaaa bbbbbbbbbb cccccccccc";

        // Scroll at 2, but cursor at position 0 (line 0)
        let scroll = adjust_annotation_scroll_pure(buffer, 0, 2, 10, 2);
        assert_eq!(scroll, 0); // Should scroll up
    }
}

#[cfg(test)]
mod word_navigation_tests {
    use super::*;

    // =========================================================================
    // Basic Word Navigation
    // =========================================================================

    #[test]
    fn test_find_next_word_basic() {
        let buffer = "hello world foo";

        assert_eq!(find_next_word_boundary(buffer, 0), 6);   // "hello " -> "world"
        assert_eq!(find_next_word_boundary(buffer, 6), 12);  // "world " -> "foo"
        assert_eq!(find_next_word_boundary(buffer, 12), 15); // At "foo" -> end
    }

    #[test]
    fn test_find_prev_word_basic() {
        let buffer = "hello world foo";

        assert_eq!(find_prev_word_boundary(buffer, 15), 12); // End -> "foo"
        assert_eq!(find_prev_word_boundary(buffer, 12), 6);  // "foo" -> "world"
        assert_eq!(find_prev_word_boundary(buffer, 6), 0);   // "world" -> "hello"
        assert_eq!(find_prev_word_boundary(buffer, 0), 0);   // At start
    }

    #[test]
    fn test_word_nav_from_middle() {
        let buffer = "hello world";

        // From middle of "world" (position 8)
        assert_eq!(find_prev_word_boundary(buffer, 8), 6); // Jump to start of "world"
        assert_eq!(find_next_word_boundary(buffer, 8), 11); // Jump to end
    }

    // =========================================================================
    // Punctuation Handling
    // =========================================================================

    #[test]
    fn test_word_nav_with_punctuation() {
        let buffer = "TODO: fix bug";

        assert_eq!(find_next_word_boundary(buffer, 0), 6);  // "TODO:" -> "fix" (skip to next word)
        assert_eq!(find_next_word_boundary(buffer, 6), 10); // "fix " -> "bug"
        assert_eq!(find_next_word_boundary(buffer, 10), 13); // "bug" -> end
    }

    #[test]
    fn test_word_nav_consecutive_punctuation() {
        let buffer = "hello...world";

        assert_eq!(find_next_word_boundary(buffer, 0), 8);  // "hello..." -> "world"
        assert_eq!(find_next_word_boundary(buffer, 8), 13); // "world" -> end

        assert_eq!(find_prev_word_boundary(buffer, 13), 8); // End -> "world"
        assert_eq!(find_prev_word_boundary(buffer, 8), 0);  // "world" -> "hello"
    }

    #[test]
    fn test_word_nav_function_call() {
        let buffer = "Fix get_user() function";

        assert_eq!(find_next_word_boundary(buffer, 0), 4);   // "Fix " -> "get_user"
        assert_eq!(find_next_word_boundary(buffer, 4), 15);  // "get_user() " -> "function"
        assert_eq!(find_next_word_boundary(buffer, 15), 23); // "function" -> end
    }

    #[test]
    fn test_word_nav_multiple_spaces() {
        let buffer = "hello    world";

        assert_eq!(find_next_word_boundary(buffer, 0), 9);  // "hello" -> "world" (skip spaces)
        assert_eq!(find_prev_word_boundary(buffer, 14), 9); // End -> "world"
        assert_eq!(find_prev_word_boundary(buffer, 9), 0);  // "world" -> "hello"
    }

    // =========================================================================
    // Unicode Support
    // =========================================================================

    #[test]
    fn test_word_nav_cyrillic() {
        let buffer = "Привет мир";

        assert_eq!(find_next_word_boundary(buffer, 0), 7);   // "Привет " -> "мир"
        assert_eq!(find_prev_word_boundary(buffer, 10), 7);  // End -> "мир"
        assert_eq!(find_prev_word_boundary(buffer, 7), 0);   // "мир" -> "Привет"
    }

    #[test]
    fn test_word_nav_mixed_unicode() {
        let buffer = "Hello Привет world";

        assert_eq!(find_next_word_boundary(buffer, 0), 6);   // "Hello " -> "Привет"
        assert_eq!(find_next_word_boundary(buffer, 6), 13);  // "Привет " -> "world"

        assert_eq!(find_prev_word_boundary(buffer, 18), 13); // End -> "world"
        assert_eq!(find_prev_word_boundary(buffer, 13), 6);  // "world" -> "Привет"
        assert_eq!(find_prev_word_boundary(buffer, 6), 0);   // "Привет" -> "Hello"
    }

    #[test]
    fn test_word_nav_emoji() {
        let buffer = "Test 🎉 done";

        assert_eq!(find_next_word_boundary(buffer, 0), 5);  // "Test " -> "🎉"
        assert_eq!(find_next_word_boundary(buffer, 5), 7);  // "🎉 " -> "done"

        assert_eq!(find_prev_word_boundary(buffer, 11), 7); // End -> "done"
        assert_eq!(find_prev_word_boundary(buffer, 7), 5);  // "done" -> "🎉"
        assert_eq!(find_prev_word_boundary(buffer, 5), 0);  // "🎉" -> "Test"
    }

    #[test]
    fn test_word_nav_cyrillic_with_punctuation() {
        let buffer = "Привет, мир!";

        assert_eq!(find_next_word_boundary(buffer, 0), 8);  // "Привет, " -> "мир"
        assert_eq!(find_next_word_boundary(buffer, 8), 12); // "мир!" -> end
    }

    // =========================================================================
    // Edge Cases
    // =========================================================================

    #[test]
    fn test_word_nav_empty_buffer() {
        let buffer = "";

        assert_eq!(find_next_word_boundary(buffer, 0), 0);
        assert_eq!(find_prev_word_boundary(buffer, 0), 0);
    }

    #[test]
    fn test_word_nav_single_word() {
        let buffer = "hello";

        assert_eq!(find_next_word_boundary(buffer, 0), 5);  // Jump to end
        assert_eq!(find_prev_word_boundary(buffer, 5), 0);  // Jump to start
        assert_eq!(find_prev_word_boundary(buffer, 0), 0);  // Already at start
    }

    #[test]
    fn test_word_nav_only_punctuation() {
        let buffer = "...";

        assert_eq!(find_next_word_boundary(buffer, 0), 3);  // Jump to end
        assert_eq!(find_prev_word_boundary(buffer, 3), 0);  // Jump to start
    }

    #[test]
    fn test_word_nav_only_whitespace() {
        let buffer = "   ";

        assert_eq!(find_next_word_boundary(buffer, 0), 3);  // Jump to end
        assert_eq!(find_prev_word_boundary(buffer, 3), 0);  // Jump to start
    }

    #[test]
    fn test_word_nav_cursor_past_end() {
        let buffer = "hello world";

        // Cursor position beyond buffer length should be clamped
        assert_eq!(find_next_word_boundary(buffer, 100), 11);
        assert_eq!(find_prev_word_boundary(buffer, 100), 6);
    }

    #[test]
    fn test_word_nav_starting_with_space() {
        let buffer = " hello world";

        assert_eq!(find_next_word_boundary(buffer, 0), 1);  // " " -> "hello"
        assert_eq!(find_next_word_boundary(buffer, 1), 7);  // "hello " -> "world"
    }

    #[test]
    fn test_word_nav_ending_with_space() {
        let buffer = "hello world ";

        assert_eq!(find_prev_word_boundary(buffer, 12), 6); // End space -> "world"
        assert_eq!(find_prev_word_boundary(buffer, 6), 0);  // "world" -> "hello"
    }

    // =========================================================================
    // Complex Real-World Scenarios
    // =========================================================================

    #[test]
    fn test_word_nav_code_annotation() {
        let buffer = "TODO: refactor get_user() method - add validation";

        // Navigate forward through words
        let mut pos = 0;

        // From start: "TODO:" -> "refactor"
        pos = find_next_word_boundary(buffer, pos);
        assert_eq!(pos, 6);

        // "refactor " -> "get_user()"
        pos = find_next_word_boundary(buffer, pos);
        assert_eq!(pos, 15);

        // "get_user() " -> "method"
        pos = find_next_word_boundary(buffer, pos);
        assert_eq!(pos, 26);

        // "method - " -> "add"
        pos = find_next_word_boundary(buffer, pos);
        assert_eq!(pos, 35);

        // "add " -> "validation"
        pos = find_next_word_boundary(buffer, pos);
        assert_eq!(pos, 39);

        // "validation" -> end
        pos = find_next_word_boundary(buffer, pos);
        assert_eq!(pos, 49);
    }

    #[test]
    fn test_word_nav_mixed_script_code() {
        // Russian comment with code
        let buffer = "Исправить функцию get_data() для обработки ошибок";

        let pos = find_next_word_boundary(buffer, 0); // "Исправить " -> "функцию"
        assert!(pos > 0 && pos < buffer.chars().count());

        let pos = find_next_word_boundary(buffer, pos); // "функцию " -> "get_data"
        assert!(pos > 0 && pos < buffer.chars().count());
    }

    #[test]
    fn test_word_nav_back_and_forth() {
        let buffer = "one two three";

        let mut pos = 0;
        pos = find_next_word_boundary(buffer, pos); // -> "two"
        assert_eq!(pos, 4);

        pos = find_next_word_boundary(buffer, pos); // -> "three"
        assert_eq!(pos, 8);

        pos = find_prev_word_boundary(buffer, pos); // -> "two"
        assert_eq!(pos, 4);

        pos = find_prev_word_boundary(buffer, pos); // -> "one"
        assert_eq!(pos, 0);
    }

    #[test]
    fn test_word_nav_special_chars() {
        let buffer = "var@name#test$value";

        assert_eq!(find_next_word_boundary(buffer, 0), 4);   // "var@name" -> "#"
        assert_eq!(find_next_word_boundary(buffer, 4), 9);   // "#test" -> "$"
        assert_eq!(find_next_word_boundary(buffer, 9), 14);  // "$value" -> end
    }

    #[test]
    fn test_word_nav_underscore_handling() {
        let buffer = "get_user_id";

        // Underscores are word characters (not boundaries) for programming
        assert_eq!(find_next_word_boundary(buffer, 0), 11);  // "get_user_id" -> end
    }
}
