//! Idle mode event handler.
//!
//! Handles keyboard events when the editor is in idle state (normal editing mode).

use super::matches_ctrl_key;
use crate::diff::adjust_diff_scroll;
use crate::models::{Action, Line, ViewMode};
use crate::navigation::{adjust_normal_scroll, find_next_annotation, find_prev_annotation};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal;
use std::io;

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
    // Check Ctrl+char hotkeys with multi-layout support first
    // Quit (Ctrl+X): English 'x', Russian 'ч'
    if matches_ctrl_key(&key, &['x', 'ч']) {
        // Caller checks if modified and decides between Exit and ShowQuitPrompt
        return Ok(IdleModeResult::ShowQuitPrompt);
    }
    // Save (Ctrl+O): English 'o', Russian 'щ'
    if matches_ctrl_key(&key, &['o', 'щ']) {
        // Caller handles save directly
        return Ok(IdleModeResult::Continue);
    }
    // Search (Ctrl+W): English 'w', Russian 'ц'
    if matches_ctrl_key(&key, &['w', 'ц']) {
        return Ok(IdleModeResult::EnterSearch);
    }
    // Toggle theme (Ctrl+T): English 't', Russian 'е'
    if matches_ctrl_key(&key, &['t', 'е']) {
        *theme = match *theme {
            crate::theme::Theme::Dark => crate::theme::Theme::Light,
            crate::theme::Theme::Light => crate::theme::Theme::Dark,
        };
        return Ok(IdleModeResult::Continue);
    }
    // Help (Ctrl+G): English 'g', Russian 'п'
    if matches_ctrl_key(&key, &['g', 'п']) {
        return Ok(IdleModeResult::ShowHelp);
    }
    // Toggle diff view (Ctrl+D): English 'd', Russian 'в'
    if matches_ctrl_key(&key, &['d', 'в']) {
        return Ok(IdleModeResult::ToggleDiffView);
    }
    // Undo (Ctrl+Z): English 'z', Russian 'я'
    if matches_ctrl_key(&key, &['z', 'я']) {
        return Ok(IdleModeResult::Undo);
    }
    // Redo (Ctrl+Y): English 'y', Russian 'н'
    if matches_ctrl_key(&key, &['y', 'н']) {
        return Ok(IdleModeResult::Redo);
    }
    // Next annotation (Ctrl+N): English 'n', Russian 'т'
    if matches_ctrl_key(&key, &['n', 'т']) {
        if let Some(next) = find_next_annotation(lines, *cursor_line) {
            *cursor_line = next;
            *annotation_scroll = 0;
            adjust_scroll_unified(*cursor_line, scroll_offset, lines, view_mode)?;
        }
        return Ok(IdleModeResult::Continue);
    }
    // Previous annotation (Ctrl+P): English 'p', Russian 'з'
    if matches_ctrl_key(&key, &['p', 'з']) {
        if let Some(prev) = find_prev_annotation(lines, *cursor_line) {
            *cursor_line = prev;
            *annotation_scroll = 0;
            adjust_scroll_unified(*cursor_line, scroll_offset, lines, view_mode)?;
        }
        return Ok(IdleModeResult::Continue);
    }

    // Non-Ctrl hotkeys use match as before
    match (key.code, key.modifiers) {
        // Delete annotation (Delete or Backspace key)
        (KeyCode::Delete, _) | (KeyCode::Backspace, _) => {
            if let Some(old_text) = &lines[*cursor_line].annotation {
                return Ok(IdleModeResult::Action(Action::EditAnnotation {
                    line_index: *cursor_line,
                    old_text: Some(old_text.clone()),
                    new_text: None,
                }));
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
    use crate::diff::{DiffLine, DiffResult, LineChange};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn test_idle_mode_jump_to_next_annotation() {
        let mut lines = vec![
            Line {
                content: "0".to_string(),
                annotation: None,
            },
            Line {
                content: "1".to_string(),
                annotation: Some("a1".to_string()),
            },
            Line {
                content: "2".to_string(),
                annotation: None,
            },
            Line {
                content: "3".to_string(),
                annotation: Some("a2".to_string()),
            },
            Line {
                content: "4".to_string(),
                annotation: None,
            },
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
        )
        .unwrap();
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
        )
        .unwrap();
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
        )
        .unwrap();
        assert_eq!(cursor_line, 3);
    }

    #[test]
    fn test_idle_mode_jump_to_prev_annotation() {
        let mut lines = vec![
            Line {
                content: "0".to_string(),
                annotation: None,
            },
            Line {
                content: "1".to_string(),
                annotation: Some("a1".to_string()),
            },
            Line {
                content: "2".to_string(),
                annotation: None,
            },
            Line {
                content: "3".to_string(),
                annotation: Some("a2".to_string()),
            },
            Line {
                content: "4".to_string(),
                annotation: None,
            },
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
        )
        .unwrap();
        assert_eq!(cursor_line, 1);
    }

    #[test]
    fn test_idle_mode_ctrl_x_shows_quit_prompt() {
        let mut lines = vec![Line {
            content: "line1".to_string(),
            annotation: None,
        }];
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
        )
        .unwrap();

        assert!(matches!(result, IdleModeResult::ShowQuitPrompt));
    }

    #[test]
    fn test_idle_mode_ctrl_g_shows_help() {
        let mut lines = vec![Line {
            content: "line1".to_string(),
            annotation: None,
        }];
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
        )
        .unwrap();

        assert!(matches!(result, IdleModeResult::ShowHelp));
    }

    #[test]
    fn test_idle_mode_ctrl_w_enters_search() {
        let mut lines = vec![Line {
            content: "line1".to_string(),
            annotation: None,
        }];
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
        )
        .unwrap();

        assert!(matches!(result, IdleModeResult::EnterSearch));
    }

    #[test]
    fn test_idle_mode_enter_enters_annotation() {
        let mut lines = vec![Line {
            content: "line1".to_string(),
            annotation: None,
        }];
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
        )
        .unwrap();

        assert!(matches!(result, IdleModeResult::EnterAnnotation { .. }));
    }

    #[test]
    fn test_idle_mode_delete_removes_annotation() {
        let mut lines = vec![Line {
            content: "line1".to_string(),
            annotation: Some("test".to_string()),
        }];
        let mut cursor_line = 0;
        let view_mode = ViewMode::Normal;
        let mut theme = crate::theme::Theme::Dark;
        let mut annotation_scroll = 0;
        let mut scroll_offset = 0;

        let result = handle_idle_mode(
            KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE),
            &mut lines,
            &mut cursor_line,
            &view_mode,
            &mut theme,
            &mut annotation_scroll,
            &mut scroll_offset,
        )
        .unwrap();

        assert!(matches!(
            result,
            IdleModeResult::Action(Action::EditAnnotation { .. })
        ));
    }

    #[test]
    fn test_idle_mode_ctrl_d_toggles_diff() {
        let mut lines = vec![Line {
            content: "line1".to_string(),
            annotation: None,
        }];
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
        )
        .unwrap();

        assert!(matches!(result, IdleModeResult::ToggleDiffView));
    }

    #[test]
    fn test_idle_mode_ctrl_z_undo() {
        let mut lines = vec![Line {
            content: "line1".to_string(),
            annotation: None,
        }];
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
        )
        .unwrap();

        assert!(matches!(result, IdleModeResult::Undo));
    }

    #[test]
    fn test_idle_mode_ctrl_y_redo() {
        let mut lines = vec![Line {
            content: "line1".to_string(),
            annotation: None,
        }];
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
        )
        .unwrap();

        assert!(matches!(result, IdleModeResult::Redo));
    }

    // ========================================================================
    // Diff View Mode Tests (same handler, different view mode)
    // ========================================================================

    #[test]
    fn test_diff_view_ctrl_x_shows_quit_prompt() {
        let mut lines = vec![Line {
            content: "line1".to_string(),
            annotation: None,
        }];
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
        )
        .unwrap();

        assert!(matches!(result, IdleModeResult::ShowQuitPrompt));
    }

    #[test]
    fn test_diff_view_ctrl_g_shows_help() {
        let mut lines = vec![Line {
            content: "line1".to_string(),
            annotation: None,
        }];
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
        )
        .unwrap();

        assert!(matches!(result, IdleModeResult::ShowHelp));
    }

    #[test]
    fn test_diff_view_delete_removes_annotation() {
        let mut lines = vec![Line {
            content: "line1".to_string(),
            annotation: Some("test".to_string()),
        }];
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
            KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE),
            &mut lines,
            &mut cursor_line,
            &view_mode,
            &mut theme,
            &mut annotation_scroll,
            &mut scroll_offset,
        )
        .unwrap();

        assert!(matches!(
            result,
            IdleModeResult::Action(Action::EditAnnotation { .. })
        ));
    }

    #[test]
    fn test_diff_view_enter_enters_annotation() {
        let mut lines = vec![Line {
            content: "line1".to_string(),
            annotation: None,
        }];
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
        )
        .unwrap();

        assert!(matches!(result, IdleModeResult::EnterAnnotation { .. }));
    }

    #[test]
    fn test_diff_view_esc_exits_diff() {
        let mut lines = vec![Line {
            content: "line1".to_string(),
            annotation: None,
        }];
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
        )
        .unwrap();

        assert!(matches!(result, IdleModeResult::ExitDiffView));
    }

    #[test]
    fn test_diff_view_ctrl_d_toggles_diff() {
        let mut lines = vec![Line {
            content: "line1".to_string(),
            annotation: None,
        }];
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
        )
        .unwrap();

        assert!(matches!(result, IdleModeResult::ToggleDiffView));
    }

    // ========================================================================
    // Independence Tests: ViewMode does NOT affect EditorState behavior
    // ========================================================================

    #[test]
    fn test_shortcuts_work_same_in_normal_and_diff_view() {
        // Test that the same shortcut produces the same result regardless of view mode
        let mut lines = vec![Line {
            content: "line1".to_string(),
            annotation: None,
        }];
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
        )
        .unwrap();

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
        )
        .unwrap();

        // Both should return ShowHelp
        assert!(matches!(result_normal, IdleModeResult::ShowHelp));
        assert!(matches!(result_diff, IdleModeResult::ShowHelp));
    }

    // ========================================================================
    // Edge Cases for Cursor Positions
    // ========================================================================

    #[test]
    fn test_cursor_position_after_delete_annotation() {
        let mut lines = vec![
            Line {
                content: "line1".to_string(),
                annotation: Some("test".to_string()),
            },
            Line {
                content: "line2".to_string(),
                annotation: None,
            },
        ];
        let mut cursor_line = 0;
        let mut theme = crate::theme::Theme::Dark;
        let mut annotation_scroll = 0;
        let mut scroll_offset = 0;

        // Delete annotation with Delete key
        let result = handle_idle_mode(
            KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE),
            &mut lines,
            &mut cursor_line,
            &ViewMode::Normal,
            &mut theme,
            &mut annotation_scroll,
            &mut scroll_offset,
        )
        .unwrap();

        assert!(matches!(result, IdleModeResult::Action(_)));
        assert_eq!(cursor_line, 0); // Cursor should stay on same line
    }

    #[test]
    fn test_cursor_position_after_navigation() {
        let mut lines = vec![
            Line {
                content: "line1".to_string(),
                annotation: Some("a1".to_string()),
            },
            Line {
                content: "line2".to_string(),
                annotation: None,
            },
            Line {
                content: "line3".to_string(),
                annotation: Some("a3".to_string()),
            },
        ];
        let mut cursor_line = 0;
        let mut theme = crate::theme::Theme::Dark;
        let mut annotation_scroll = 0;
        let mut scroll_offset = 0;

        // Jump to next annotation (Ctrl+N)
        handle_idle_mode(
            KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL),
            &mut lines,
            &mut cursor_line,
            &ViewMode::Normal,
            &mut theme,
            &mut annotation_scroll,
            &mut scroll_offset,
        )
        .unwrap();

        assert_eq!(cursor_line, 2); // Should jump to line 3 (index 2)
    }

    #[test]
    fn test_cursor_position_at_file_boundaries() {
        let mut lines = vec![Line {
            content: "only line".to_string(),
            annotation: None,
        }];
        let mut cursor_line = 0;
        let mut theme = crate::theme::Theme::Dark;
        let mut annotation_scroll = 0;
        let mut scroll_offset = 0;

        // Try to move down when already at end
        handle_idle_mode(
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            &mut lines,
            &mut cursor_line,
            &ViewMode::Normal,
            &mut theme,
            &mut annotation_scroll,
            &mut scroll_offset,
        )
        .unwrap();

        assert_eq!(cursor_line, 0); // Should stay at 0

        // Try to move up when already at start
        handle_idle_mode(
            KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
            &mut lines,
            &mut cursor_line,
            &ViewMode::Normal,
            &mut theme,
            &mut annotation_scroll,
            &mut scroll_offset,
        )
        .unwrap();

        assert_eq!(cursor_line, 0); // Should stay at 0
    }

    #[test]
    fn test_empty_lines_cursor_behavior() {
        let mut lines = vec![];
        let mut cursor_line = 0;
        let mut theme = crate::theme::Theme::Dark;
        let mut annotation_scroll = 0;
        let mut scroll_offset = 0;

        // Navigate down in empty file
        let result = handle_idle_mode(
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            &mut lines,
            &mut cursor_line,
            &ViewMode::Normal,
            &mut theme,
            &mut annotation_scroll,
            &mut scroll_offset,
        )
        .unwrap();

        assert!(matches!(result, IdleModeResult::Continue));
        // Cursor should remain valid (0)
        assert_eq!(cursor_line, 0);
    }
}
