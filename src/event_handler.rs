#![allow(clippy::too_many_arguments)]
use crate::diff::adjust_diff_scroll;
use crate::models::{Action, Line, ViewMode};
use crate::navigation::{
    adjust_annotation_scroll_pure, adjust_normal_scroll, find_matches, find_next_annotation,
    find_prev_annotation, move_cursor_down_in_wrapped, move_cursor_up_in_wrapped,
};
use crossterm::{
    event::{KeyCode, KeyEvent, KeyModifiers},
    terminal,
};
use std::io;

// ============================================================================
// New Unified Architecture: IdleModeResult + handle_idle_mode
// ============================================================================

/// Result of handling key events in Idle state.
/// Works the same regardless of ViewMode (Normal or Diff).
#[allow(dead_code)]
pub enum IdleModeResult {
    /// Continue in current state
    Continue,
    /// Exit the editor immediately (no unsaved changes)
    Exit,
    /// Perform undo
    Undo,
    /// Perform redo
    Redo,
    /// An action was performed (e.g., delete annotation)
    Action(Action),
    /// Enter annotation editing mode
    EnterAnnotation { initial_text: String },
    /// Enter search mode
    EnterSearch,
    /// Show help overlay
    ShowHelp,
    /// Show quit prompt (unsaved changes)
    ShowQuitPrompt,
    /// Toggle diff view mode
    ToggleDiffView,
    /// Exit diff view (only valid when in diff view mode)
    ExitDiffView,
}

/// Handles key events in Idle state.
/// This is the unified handler that works the same in both Normal and Diff view modes.
/// The view_mode parameter is only used for scroll adjustment (diff needs synchronized scroll).
pub fn handle_idle_mode(
    key: KeyEvent,
    lines: &mut [Line],
    cursor_line: &mut usize,
    view_mode: &ViewMode,
    theme: &mut crate::theme::Theme,
    annotation_scroll: &mut usize,
    scroll_offset: &mut usize,
) -> io::Result<IdleModeResult> {
    match (key.code, key.modifiers) {
        // Quit (Ctrl+X)
        (KeyCode::Char('x'), KeyModifiers::CONTROL) => {
            // Caller checks if modified and decides between Exit and ShowQuitPrompt
            return Ok(IdleModeResult::ShowQuitPrompt);
        }
        // Save (Ctrl+O) - handled by caller
        (KeyCode::Char('o'), KeyModifiers::CONTROL) => {
            // Caller handles save directly
            return Ok(IdleModeResult::Continue);
        }
        // Search (Ctrl+W)
        (KeyCode::Char('w'), KeyModifiers::CONTROL) => {
            return Ok(IdleModeResult::EnterSearch);
        }
        // Toggle theme (Ctrl+T)
        (KeyCode::Char('t'), KeyModifiers::CONTROL) => {
            *theme = match *theme {
                crate::theme::Theme::Dark => crate::theme::Theme::Light,
                crate::theme::Theme::Light => crate::theme::Theme::Dark,
            };
        }
        // Help (Ctrl+G)
        (KeyCode::Char('g'), KeyModifiers::CONTROL) => {
            return Ok(IdleModeResult::ShowHelp);
        }
        // Toggle diff view (Ctrl+K)
        (KeyCode::Char('k'), KeyModifiers::CONTROL) => {
            return Ok(IdleModeResult::ToggleDiffView);
        }
        // Undo (Ctrl+Z)
        (KeyCode::Char('z'), KeyModifiers::CONTROL) => {
            return Ok(IdleModeResult::Undo);
        }
        // Redo (Ctrl+Y)
        (KeyCode::Char('y'), KeyModifiers::CONTROL) => {
            return Ok(IdleModeResult::Redo);
        }
        // Delete annotation (Ctrl+D)
        (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
            if let Some(old_text) = &lines[*cursor_line].annotation {
                return Ok(IdleModeResult::Action(Action::EditAnnotation {
                    line_index: *cursor_line,
                    old_text: Some(old_text.clone()),
                    new_text: None,
                }));
            }
        }
        // Next annotation (Ctrl+N)
        (KeyCode::Char('n'), KeyModifiers::CONTROL) => {
            if let Some(next) = find_next_annotation(lines, *cursor_line) {
                *cursor_line = next;
                *annotation_scroll = 0;
                adjust_scroll_unified(*cursor_line, scroll_offset, lines, view_mode)?;
            }
        }
        // Previous annotation (Ctrl+P)
        (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
            if let Some(prev) = find_prev_annotation(lines, *cursor_line) {
                *cursor_line = prev;
                *annotation_scroll = 0;
                adjust_scroll_unified(*cursor_line, scroll_offset, lines, view_mode)?;
            }
        }
        // Page Up
        (KeyCode::PageUp, _) | (KeyCode::Up, KeyModifiers::ALT) => {
            let (_, height) = terminal::size()?;
            let content_height = (height.saturating_sub(5)) as usize;
            *cursor_line = cursor_line.saturating_sub(content_height);
            *annotation_scroll = 0;
            adjust_scroll_unified(*cursor_line, scroll_offset, lines, view_mode)?;
        }
        // Page Down
        (KeyCode::PageDown, _) | (KeyCode::Down, KeyModifiers::ALT) => {
            let (_, height) = terminal::size()?;
            let content_height = (height.saturating_sub(5)) as usize;
            *cursor_line = (*cursor_line + content_height).min(lines.len().saturating_sub(1));
            *annotation_scroll = 0;
            adjust_scroll_unified(*cursor_line, scroll_offset, lines, view_mode)?;
        }
        // Up arrow
        (KeyCode::Up, _) => {
            if *cursor_line > 0 {
                *cursor_line -= 1;
                *annotation_scroll = 0;
                adjust_scroll_unified(*cursor_line, scroll_offset, lines, view_mode)?;
            }
        }
        // Down arrow
        (KeyCode::Down, _) => {
            if *cursor_line < lines.len().saturating_sub(1) {
                *cursor_line += 1;
                *annotation_scroll = 0;
                adjust_scroll_unified(*cursor_line, scroll_offset, lines, view_mode)?;
            }
        }
        // Enter annotation mode
        (KeyCode::Enter, _) => {
            let initial_text = lines[*cursor_line].annotation.clone().unwrap_or_default();
            *annotation_scroll = 0;
            return Ok(IdleModeResult::EnterAnnotation { initial_text });
        }
        // Escape - only meaningful in diff view (exits diff)
        (KeyCode::Esc, _) => {
            if matches!(view_mode, ViewMode::Diff { .. }) {
                return Ok(IdleModeResult::ExitDiffView);
            }
        }
        _ => {}
    }
    Ok(IdleModeResult::Continue)
}

/// Unified scroll adjustment that works for both Normal and Diff view modes.
fn adjust_scroll_unified(
    cursor_line: usize,
    scroll_offset: &mut usize,
    lines: &[Line],
    view_mode: &ViewMode,
) -> io::Result<()> {
    let (width, height) = terminal::size().unwrap_or((80, 24));
    let content_height = (height.saturating_sub(5)) as usize;

    match view_mode {
        ViewMode::Diff { diff_result } => {
            // Use diff-aware scroll adjustment
            *scroll_offset = adjust_diff_scroll(cursor_line, *scroll_offset, content_height, diff_result);
        }
        ViewMode::Normal => {
            // Use normal scroll adjustment
            *scroll_offset = adjust_normal_scroll(cursor_line, *scroll_offset, content_height, lines, width as usize);
        }
    }

    Ok(())
}

// ============================================================================
// Annotation Mode Handler (New Architecture)
// ============================================================================

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
    Ok(AnnotationModeResult::Continue)
}

// ============================================================================
// Search Mode Handler (New Architecture)
// ============================================================================

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
) -> io::Result<SearchModeResult> {
    match (key.code, key.modifiers) {
        (KeyCode::Enter, KeyModifiers::SHIFT) => {
            // Shift+Enter: previous match
            if !search_matches.is_empty() {
                prev_search_match(search_matches, current_match, cursor_line);
                adjust_scroll_unified(*cursor_line, scroll_offset, lines, view_mode)?;
            }
        }
        (KeyCode::Enter, _) => {
            // Enter: next match
            if !search_matches.is_empty() {
                next_search_match(search_matches, current_match, cursor_line);
                adjust_scroll_unified(*cursor_line, scroll_offset, lines, view_mode)?;
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
            adjust_scroll_unified(*cursor_line, scroll_offset, lines, view_mode)?;
        }
        (KeyCode::Backspace, _) => {
            if *cursor_pos > 0 {
                *cursor_pos -= 1;
                query.remove(*cursor_pos);
                perform_search(query, lines, search_matches, current_match, cursor_line);
                adjust_scroll_unified(*cursor_line, scroll_offset, lines, view_mode)?;
            }
        }
        _ => {}
    }
    Ok(SearchModeResult::Continue)
}

// ============================================================================
// Quit Prompt Handler
// ============================================================================

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

// ============================================================================
// Helper Functions
// ============================================================================

fn move_cursor_up(buffer: &str, cursor_pos: &mut usize, annotation_scroll: &mut usize) -> io::Result<()> {
    let (width, _) = terminal::size()?;
    let max_width = width as usize - 4;

    // Use pure function from navigation module
    *cursor_pos = move_cursor_up_in_wrapped(buffer, *cursor_pos, max_width);

    // Adjust scroll to keep cursor visible
    adjust_annotation_scroll(buffer, *cursor_pos, annotation_scroll)?;

    Ok(())
}

fn move_cursor_down(buffer: &str, cursor_pos: &mut usize, annotation_scroll: &mut usize) -> io::Result<()> {
    let (width, _) = terminal::size()?;
    let max_width = width as usize - 4;

    // Use pure function from navigation module
    *cursor_pos = move_cursor_down_in_wrapped(buffer, *cursor_pos, max_width);

    // Adjust scroll to keep cursor visible
    adjust_annotation_scroll(buffer, *cursor_pos, annotation_scroll)?;

    Ok(())
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
    use crate::navigation::{cycle_match, CycleDirection};

    if let Some((new_idx, line)) = cycle_match(search_matches, *current_match, CycleDirection::Next) {
        *current_match = Some(new_idx);
        *cursor_line = line;
    }
}

fn prev_search_match(
    search_matches: &[usize],
    current_match: &mut Option<usize>,
    cursor_line: &mut usize,
) {
    use crate::navigation::{cycle_match, CycleDirection};

    if let Some((new_idx, line)) = cycle_match(search_matches, *current_match, CycleDirection::Previous) {
        *current_match = Some(new_idx);
        *cursor_line = line;
    }
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
    use crate::models::Line;
    use crate::diff::{DiffResult, DiffLine, LineChange};
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
    // Idle Mode Tests (using new handle_idle_mode)
    // ========================================================================

    #[test]
    fn test_idle_mode_jump_to_next_annotation() {
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
        let view_mode = ViewMode::Normal;
        let mut theme = crate::theme::Theme::Dark;

        // Jump Next (from 0 to 1)
        let _ = handle_idle_mode(
            KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL),
            &mut lines,
            &mut cursor_line,
            &view_mode,
            &mut theme,
            &mut annotation_scroll,
            &mut scroll_offset,
        ).unwrap();
        assert_eq!(cursor_line, 1);

        // Jump Next (from 1 to 3)
        let _ = handle_idle_mode(
            KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL),
            &mut lines,
            &mut cursor_line,
            &view_mode,
            &mut theme,
            &mut annotation_scroll,
            &mut scroll_offset,
        ).unwrap();
        assert_eq!(cursor_line, 3);

        // Jump Next (from 3 - no next, stays at 3)
        let _ = handle_idle_mode(
            KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL),
            &mut lines,
            &mut cursor_line,
            &view_mode,
            &mut theme,
            &mut annotation_scroll,
            &mut scroll_offset,
        ).unwrap();
        assert_eq!(cursor_line, 3);
    }

    #[test]
    fn test_idle_mode_jump_to_prev_annotation() {
        let mut lines = vec![
            Line { content: "0".to_string(), annotation: None },
            Line { content: "1".to_string(), annotation: Some("a1".to_string()) },
            Line { content: "2".to_string(), annotation: None },
            Line { content: "3".to_string(), annotation: Some("a2".to_string()) },
            Line { content: "4".to_string(), annotation: None },
        ];

        let mut cursor_line = 3;
        let mut scroll_offset = 0;
        let mut annotation_scroll = 0;
        let view_mode = ViewMode::Normal;
        let mut theme = crate::theme::Theme::Dark;

        // Jump Prev (from 3 to 1)
        let _ = handle_idle_mode(
            KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL),
            &mut lines,
            &mut cursor_line,
            &view_mode,
            &mut theme,
            &mut annotation_scroll,
            &mut scroll_offset,
        ).unwrap();
        assert_eq!(cursor_line, 1);
    }

    #[test]
    fn test_idle_mode_ctrl_x_shows_quit_prompt() {
        let mut lines = vec![
            Line { content: "line1".to_string(), annotation: None },
        ];
        let mut cursor_line = 0;
        let view_mode = ViewMode::Normal;
        let mut theme = crate::theme::Theme::Dark;
        let mut annotation_scroll = 0;
        let mut scroll_offset = 0;

        let result = handle_idle_mode(
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
            &mut lines,
            &mut cursor_line,
            &view_mode,
            &mut theme,
            &mut annotation_scroll,
            &mut scroll_offset,
        ).unwrap();

        assert!(matches!(result, IdleModeResult::ShowQuitPrompt));
    }

    #[test]
    fn test_idle_mode_ctrl_g_shows_help() {
        let mut lines = vec![
            Line { content: "line1".to_string(), annotation: None },
        ];
        let mut cursor_line = 0;
        let view_mode = ViewMode::Normal;
        let mut theme = crate::theme::Theme::Dark;
        let mut annotation_scroll = 0;
        let mut scroll_offset = 0;

        let result = handle_idle_mode(
            KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL),
            &mut lines,
            &mut cursor_line,
            &view_mode,
            &mut theme,
            &mut annotation_scroll,
            &mut scroll_offset,
        ).unwrap();

        assert!(matches!(result, IdleModeResult::ShowHelp));
    }

    #[test]
    fn test_idle_mode_ctrl_w_enters_search() {
        let mut lines = vec![
            Line { content: "line1".to_string(), annotation: None },
        ];
        let mut cursor_line = 0;
        let view_mode = ViewMode::Normal;
        let mut theme = crate::theme::Theme::Dark;
        let mut annotation_scroll = 0;
        let mut scroll_offset = 0;

        let result = handle_idle_mode(
            KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL),
            &mut lines,
            &mut cursor_line,
            &view_mode,
            &mut theme,
            &mut annotation_scroll,
            &mut scroll_offset,
        ).unwrap();

        assert!(matches!(result, IdleModeResult::EnterSearch));
    }

    #[test]
    fn test_idle_mode_enter_enters_annotation() {
        let mut lines = vec![
            Line { content: "line1".to_string(), annotation: None },
        ];
        let mut cursor_line = 0;
        let view_mode = ViewMode::Normal;
        let mut theme = crate::theme::Theme::Dark;
        let mut annotation_scroll = 0;
        let mut scroll_offset = 0;

        let result = handle_idle_mode(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut lines,
            &mut cursor_line,
            &view_mode,
            &mut theme,
            &mut annotation_scroll,
            &mut scroll_offset,
        ).unwrap();

        assert!(matches!(result, IdleModeResult::EnterAnnotation { .. }));
    }

    #[test]
    fn test_idle_mode_ctrl_d_deletes_annotation() {
        let mut lines = vec![
            Line { content: "line1".to_string(), annotation: Some("test".to_string()) },
        ];
        let mut cursor_line = 0;
        let view_mode = ViewMode::Normal;
        let mut theme = crate::theme::Theme::Dark;
        let mut annotation_scroll = 0;
        let mut scroll_offset = 0;

        let result = handle_idle_mode(
            KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
            &mut lines,
            &mut cursor_line,
            &view_mode,
            &mut theme,
            &mut annotation_scroll,
            &mut scroll_offset,
        ).unwrap();

        assert!(matches!(result, IdleModeResult::Action(Action::EditAnnotation { .. })));
    }

    #[test]
    fn test_idle_mode_ctrl_k_toggles_diff() {
        let mut lines = vec![
            Line { content: "line1".to_string(), annotation: None },
        ];
        let mut cursor_line = 0;
        let view_mode = ViewMode::Normal;
        let mut theme = crate::theme::Theme::Dark;
        let mut annotation_scroll = 0;
        let mut scroll_offset = 0;

        let result = handle_idle_mode(
            KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL),
            &mut lines,
            &mut cursor_line,
            &view_mode,
            &mut theme,
            &mut annotation_scroll,
            &mut scroll_offset,
        ).unwrap();

        assert!(matches!(result, IdleModeResult::ToggleDiffView));
    }

    #[test]
    fn test_idle_mode_ctrl_z_undo() {
        let mut lines = vec![
            Line { content: "line1".to_string(), annotation: None },
        ];
        let mut cursor_line = 0;
        let view_mode = ViewMode::Normal;
        let mut theme = crate::theme::Theme::Dark;
        let mut annotation_scroll = 0;
        let mut scroll_offset = 0;

        let result = handle_idle_mode(
            KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL),
            &mut lines,
            &mut cursor_line,
            &view_mode,
            &mut theme,
            &mut annotation_scroll,
            &mut scroll_offset,
        ).unwrap();

        assert!(matches!(result, IdleModeResult::Undo));
    }

    #[test]
    fn test_idle_mode_ctrl_y_redo() {
        let mut lines = vec![
            Line { content: "line1".to_string(), annotation: None },
        ];
        let mut cursor_line = 0;
        let view_mode = ViewMode::Normal;
        let mut theme = crate::theme::Theme::Dark;
        let mut annotation_scroll = 0;
        let mut scroll_offset = 0;

        let result = handle_idle_mode(
            KeyEvent::new(KeyCode::Char('y'), KeyModifiers::CONTROL),
            &mut lines,
            &mut cursor_line,
            &view_mode,
            &mut theme,
            &mut annotation_scroll,
            &mut scroll_offset,
        ).unwrap();

        assert!(matches!(result, IdleModeResult::Redo));
    }

    // ========================================================================
    // Diff View Mode Tests (same handler, different view mode)
    // ========================================================================

    #[test]
    fn test_diff_view_ctrl_x_shows_quit_prompt() {
        let mut lines = vec![
            Line { content: "line1".to_string(), annotation: None },
        ];
        let mut cursor_line = 0;
        let view_mode = ViewMode::Diff {
            diff_result: DiffResult {
                lines: vec![DiffLine {
                    working: Some((1, "line1".to_string(), LineChange::Unchanged)),
                    head: Some((1, "line1".to_string(), LineChange::Unchanged)),
                }],
            },
        };
        let mut theme = crate::theme::Theme::Dark;
        let mut annotation_scroll = 0;
        let mut scroll_offset = 0;

        let result = handle_idle_mode(
            KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL),
            &mut lines,
            &mut cursor_line,
            &view_mode,
            &mut theme,
            &mut annotation_scroll,
            &mut scroll_offset,
        ).unwrap();

        assert!(matches!(result, IdleModeResult::ShowQuitPrompt));
    }

    #[test]
    fn test_diff_view_ctrl_g_shows_help() {
        let mut lines = vec![
            Line { content: "line1".to_string(), annotation: None },
        ];
        let mut cursor_line = 0;
        let view_mode = ViewMode::Diff {
            diff_result: DiffResult {
                lines: vec![DiffLine {
                    working: Some((1, "line1".to_string(), LineChange::Unchanged)),
                    head: Some((1, "line1".to_string(), LineChange::Unchanged)),
                }],
            },
        };
        let mut theme = crate::theme::Theme::Dark;
        let mut annotation_scroll = 0;
        let mut scroll_offset = 0;

        let result = handle_idle_mode(
            KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL),
            &mut lines,
            &mut cursor_line,
            &view_mode,
            &mut theme,
            &mut annotation_scroll,
            &mut scroll_offset,
        ).unwrap();

        assert!(matches!(result, IdleModeResult::ShowHelp));
    }

    #[test]
    fn test_diff_view_ctrl_d_deletes_annotation() {
        let mut lines = vec![
            Line { content: "line1".to_string(), annotation: Some("test".to_string()) },
        ];
        let mut cursor_line = 0;
        let view_mode = ViewMode::Diff {
            diff_result: DiffResult {
                lines: vec![DiffLine {
                    working: Some((1, "line1".to_string(), LineChange::Unchanged)),
                    head: Some((1, "line1".to_string(), LineChange::Unchanged)),
                }],
            },
        };
        let mut theme = crate::theme::Theme::Dark;
        let mut annotation_scroll = 0;
        let mut scroll_offset = 0;

        let result = handle_idle_mode(
            KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
            &mut lines,
            &mut cursor_line,
            &view_mode,
            &mut theme,
            &mut annotation_scroll,
            &mut scroll_offset,
        ).unwrap();

        assert!(matches!(result, IdleModeResult::Action(Action::EditAnnotation { .. })));
    }

    #[test]
    fn test_diff_view_enter_enters_annotation() {
        let mut lines = vec![
            Line { content: "line1".to_string(), annotation: None },
        ];
        let mut cursor_line = 0;
        let view_mode = ViewMode::Diff {
            diff_result: DiffResult {
                lines: vec![DiffLine {
                    working: Some((1, "line1".to_string(), LineChange::Unchanged)),
                    head: Some((1, "line1".to_string(), LineChange::Unchanged)),
                }],
            },
        };
        let mut theme = crate::theme::Theme::Dark;
        let mut annotation_scroll = 0;
        let mut scroll_offset = 0;

        let result = handle_idle_mode(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut lines,
            &mut cursor_line,
            &view_mode,
            &mut theme,
            &mut annotation_scroll,
            &mut scroll_offset,
        ).unwrap();

        assert!(matches!(result, IdleModeResult::EnterAnnotation { .. }));
    }

    #[test]
    fn test_diff_view_esc_exits_diff() {
        let mut lines = vec![
            Line { content: "line1".to_string(), annotation: None },
        ];
        let mut cursor_line = 0;
        let view_mode = ViewMode::Diff {
            diff_result: DiffResult {
                lines: vec![DiffLine {
                    working: Some((1, "line1".to_string(), LineChange::Unchanged)),
                    head: Some((1, "line1".to_string(), LineChange::Unchanged)),
                }],
            },
        };
        let mut theme = crate::theme::Theme::Dark;
        let mut annotation_scroll = 0;
        let mut scroll_offset = 0;

        let result = handle_idle_mode(
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
            &mut lines,
            &mut cursor_line,
            &view_mode,
            &mut theme,
            &mut annotation_scroll,
            &mut scroll_offset,
        ).unwrap();

        assert!(matches!(result, IdleModeResult::ExitDiffView));
    }

    #[test]
    fn test_diff_view_ctrl_k_toggles_diff() {
        let mut lines = vec![
            Line { content: "line1".to_string(), annotation: None },
        ];
        let mut cursor_line = 0;
        let view_mode = ViewMode::Diff {
            diff_result: DiffResult {
                lines: vec![DiffLine {
                    working: Some((1, "line1".to_string(), LineChange::Unchanged)),
                    head: Some((1, "line1".to_string(), LineChange::Unchanged)),
                }],
            },
        };
        let mut theme = crate::theme::Theme::Dark;
        let mut annotation_scroll = 0;
        let mut scroll_offset = 0;

        let result = handle_idle_mode(
            KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL),
            &mut lines,
            &mut cursor_line,
            &view_mode,
            &mut theme,
            &mut annotation_scroll,
            &mut scroll_offset,
        ).unwrap();

        assert!(matches!(result, IdleModeResult::ToggleDiffView));
    }

    // ========================================================================
    // Quit Prompt Tests
    // ========================================================================

    #[test]
    fn test_quit_prompt_y_saves_and_exits() {
        let result = handle_quit_prompt(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
        assert!(matches!(result, QuitPromptResult::SaveAndExit));
    }

    #[test]
    fn test_quit_prompt_n_exits() {
        let result = handle_quit_prompt(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE));
        assert!(matches!(result, QuitPromptResult::Exit));
    }

    #[test]
    fn test_quit_prompt_esc_cancels() {
        let result = handle_quit_prompt(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(matches!(result, QuitPromptResult::Cancel));
    }

    // ========================================================================
    // Annotation Mode Tests
    // ========================================================================

    #[test]
    fn test_annotation_input_enter_saves() {
        let lines = vec![
            Line { content: "line1".to_string(), annotation: None },
        ];
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
        ).unwrap();

        assert!(matches!(result, AnnotationModeResult::Save(_)));
    }

    #[test]
    fn test_annotation_input_esc_cancels() {
        let lines = vec![
            Line { content: "line1".to_string(), annotation: None },
        ];
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
        ).unwrap();

        assert!(matches!(result, AnnotationModeResult::Cancel));
    }

    #[test]
    fn test_annotation_input_char_appends() {
        let lines = vec![
            Line { content: "line1".to_string(), annotation: None },
        ];
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
        ).unwrap();

        assert!(matches!(result, AnnotationModeResult::Continue));
        assert_eq!(buffer, "test!");
        assert_eq!(cursor_pos, 5);
    }

    #[test]
    fn test_annotation_input_backspace_removes() {
        let lines = vec![
            Line { content: "line1".to_string(), annotation: None },
        ];
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
        ).unwrap();

        assert!(matches!(result, AnnotationModeResult::Continue));
        assert_eq!(buffer, "tes");
        assert_eq!(cursor_pos, 3);
    }

    // ========================================================================
    // Search Mode Tests
    // ========================================================================

    #[test]
    fn test_search_input_esc_exits() {
        let lines = vec![
            Line { content: "test line".to_string(), annotation: None },
        ];
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
        ).unwrap();

        assert!(matches!(result, SearchModeResult::Exit));
        assert!(search_matches.is_empty());
        assert!(current_match.is_none());
    }

    #[test]
    fn test_search_input_char_updates_query() {
        let lines = vec![
            Line { content: "test line".to_string(), annotation: None },
        ];
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
        ).unwrap();

        assert!(matches!(result, SearchModeResult::Continue));
        assert_eq!(query, "test");
    }

    // ========================================================================
    // Independence Tests: ViewMode does NOT affect EditorState behavior
    // ========================================================================

    #[test]
    fn test_shortcuts_work_same_in_normal_and_diff_view() {
        // Test that the same shortcut produces the same result regardless of view mode
        let mut lines = vec![
            Line { content: "line1".to_string(), annotation: None },
        ];
        let mut cursor_line = 0;
        let mut theme = crate::theme::Theme::Dark;
        let mut annotation_scroll = 0;
        let mut scroll_offset = 0;

        // Test Ctrl+G in Normal view
        let normal_view = ViewMode::Normal;
        let result_normal = handle_idle_mode(
            KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL),
            &mut lines,
            &mut cursor_line,
            &normal_view,
            &mut theme,
            &mut annotation_scroll,
            &mut scroll_offset,
        ).unwrap();

        // Test Ctrl+G in Diff view
        let diff_view = ViewMode::Diff {
            diff_result: DiffResult {
                lines: vec![DiffLine {
                    working: Some((1, "line1".to_string(), LineChange::Unchanged)),
                    head: Some((1, "line1".to_string(), LineChange::Unchanged)),
                }],
            },
        };
        let result_diff = handle_idle_mode(
            KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL),
            &mut lines,
            &mut cursor_line,
            &diff_view,
            &mut theme,
            &mut annotation_scroll,
            &mut scroll_offset,
        ).unwrap();

        // Both should return ShowHelp
        assert!(matches!(result_normal, IdleModeResult::ShowHelp));
        assert!(matches!(result_diff, IdleModeResult::ShowHelp));
    }
}
