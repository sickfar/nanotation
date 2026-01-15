#![allow(clippy::too_many_arguments)]
use crate::diff::adjust_diff_scroll;
use crate::file_tree::FileTreePanel;
use crate::models::{Action, Line, ViewMode};
use crate::navigation::{
    adjust_annotation_scroll_pure, adjust_normal_scroll, find_matches, find_next_annotation,
    find_next_word_boundary, find_prev_annotation, find_prev_word_boundary,
    move_cursor_down_in_wrapped, move_cursor_up_in_wrapped,
};
use crossterm::{
    event::{KeyCode, KeyEvent, KeyModifiers},
    terminal,
};
use std::io;
use std::path::PathBuf;

// ============================================================================
// Multi-Hotkey Helper for Keyboard Layout Independence
// ============================================================================

/// Check if a key event matches Ctrl+<one of the alternatives>.
/// This enables hotkeys to work across different keyboard layouts (EN/RU/CN/etc.).
///
/// For example, matches_ctrl_key(&key, &['x', 'ч']) will return true if:
/// - English layout: Ctrl+X pressed (produces 'x')
/// - Russian layout: Ctrl+X physical key pressed (produces 'ч')
fn matches_ctrl_key(key: &KeyEvent, alternatives: &[char]) -> bool {
    if key.modifiers != KeyModifiers::CONTROL {
        return false;
    }
    if let KeyCode::Char(c) = key.code {
        alternatives.contains(&c)
    } else {
        false
    }
}

/// Check if a key code matches one of the character alternatives.
/// Case-insensitive: converts to lowercase before checking.
fn matches_char(key_code: &KeyCode, alternatives: &[char]) -> bool {
    if let KeyCode::Char(c) = key_code {
        let c_lower = c.to_lowercase().next().unwrap_or(*c);
        alternatives.iter().any(|&alt| {
            let alt_lower = alt.to_lowercase().next().unwrap_or(alt);
            c_lower == alt_lower
        })
    } else {
        false
    }
}

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

// ============================================================================
// Tree Panel Input Handling
// ============================================================================

/// Result of handling key events in tree panel.
pub enum TreeInputResult {
    /// Continue in current state
    Continue,
    /// Open a file (switch focus to editor)
    OpenFile(PathBuf),
    /// Tree needs refresh (file deleted, etc.)
    RefreshNeeded,
}

/// Handle key events when file tree is focused.
pub fn handle_tree_input(
    key: KeyEvent,
    tree: &mut FileTreePanel,
    terminal_height: u16,
) -> io::Result<TreeInputResult> {
    let page_size = terminal_height.saturating_sub(8) as usize;

    match key.code {
        // Navigation
        KeyCode::Up => {
            tree.navigate_up();
            Ok(TreeInputResult::Continue)
        }
        KeyCode::Down => {
            tree.navigate_down();
            Ok(TreeInputResult::Continue)
        }
        KeyCode::Home => {
            tree.navigate_home();
            Ok(TreeInputResult::Continue)
        }
        KeyCode::End => {
            tree.navigate_end();
            Ok(TreeInputResult::Continue)
        }
        KeyCode::PageUp => {
            tree.page_up(page_size);
            Ok(TreeInputResult::Continue)
        }
        KeyCode::PageDown => {
            tree.page_down(page_size);
            Ok(TreeInputResult::Continue)
        }

        // Expand/Collapse with arrow keys
        KeyCode::Right => {
            if let Err(e) = tree.expand_selected() {
                // Log error but continue
                eprintln!("Error expanding: {}", e);
            }
            Ok(TreeInputResult::Continue)
        }
        KeyCode::Left => {
            if let Err(e) = tree.collapse_selected() {
                // Log error but continue
                eprintln!("Error collapsing: {}", e);
            }
            Ok(TreeInputResult::Continue)
        }

        // Enter - open file or toggle folder
        KeyCode::Enter => {
            if let Some(entry) = tree.get_selected() {
                if entry.is_directory() {
                    // Toggle expand/collapse
                    if entry.is_expanded() {
                        let _ = tree.collapse_selected();
                    } else {
                        let _ = tree.expand_selected();
                    }
                    Ok(TreeInputResult::Continue)
                } else if let Some(path) = tree.get_selected_file_path() {
                    // Check if file still exists
                    if !path.exists() {
                        return Ok(TreeInputResult::RefreshNeeded);
                    }
                    // Open the file
                    Ok(TreeInputResult::OpenFile(path.to_path_buf()))
                } else {
                    Ok(TreeInputResult::Continue)
                }
            } else {
                Ok(TreeInputResult::Continue)
            }
        }

        _ => Ok(TreeInputResult::Continue),
    }
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
    // Yes: English 'y', Russian 'н' (QWERTY Y key position)
    if matches_char(&key.code, &['y', 'н']) {
        return QuitPromptResult::SaveAndExit;
    }
    // No: English 'n', Russian 'т' (QWERTY N key position)
    if matches_char(&key.code, &['n', 'т']) {
        return QuitPromptResult::Exit;
    }
    // Cancel: English 'c', Russian 'с' (QWERTY C key position), or Esc
    if key.code == KeyCode::Esc || matches_char(&key.code, &['c', 'с']) {
        return QuitPromptResult::Cancel;
    }
    QuitPromptResult::Continue
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

    // Tests for matches_char helper
    #[test]
    fn test_matches_char_lowercase() {
        assert!(matches_char(&KeyCode::Char('y'), &['y', 'н']));
    }

    #[test]
    fn test_matches_char_uppercase() {
        assert!(matches_char(&KeyCode::Char('Y'), &['y', 'н']));
    }

    #[test]
    fn test_matches_char_cyrillic() {
        assert!(matches_char(&KeyCode::Char('н'), &['y', 'н']));
    }

    #[test]
    fn test_matches_char_no_match() {
        assert!(!matches_char(&KeyCode::Char('x'), &['y', 'н']));
    }

    #[test]
    fn test_matches_char_not_char_keycode() {
        assert!(!matches_char(&KeyCode::Enter, &['y', 'н']));
    }

    // Tests for matches_ctrl_key helper
    #[test]
    fn test_matches_ctrl_key_latin() {
        let key = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL);
        assert!(matches_ctrl_key(&key, &['x', 'ч']));
    }

    #[test]
    fn test_matches_ctrl_key_cyrillic() {
        let key = KeyEvent::new(KeyCode::Char('ч'), KeyModifiers::CONTROL);
        assert!(matches_ctrl_key(&key, &['x', 'ч']));
    }

    #[test]
    fn test_matches_ctrl_key_no_match() {
        let key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL);
        assert!(!matches_ctrl_key(&key, &['x', 'ч']));
    }

    #[test]
    fn test_matches_ctrl_key_no_modifier() {
        let key = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
        assert!(!matches_ctrl_key(&key, &['x', 'ч']));
    }

    #[test]
    fn test_matches_ctrl_key_wrong_keycode_type() {
        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL);
        assert!(!matches_ctrl_key(&key, &['x', 'ч']));
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
    fn test_idle_mode_delete_removes_annotation() {
        let mut lines = vec![
            Line { content: "line1".to_string(), annotation: Some("test".to_string()) },
        ];
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
        ).unwrap();

        assert!(matches!(result, IdleModeResult::Action(Action::EditAnnotation { .. })));
    }

    #[test]
    fn test_idle_mode_ctrl_d_toggles_diff() {
        let mut lines = vec![
            Line { content: "line1".to_string(), annotation: None },
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
    fn test_diff_view_delete_removes_annotation() {
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
            KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE),
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
    fn test_diff_view_ctrl_d_toggles_diff() {
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
            KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL),
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

    #[test]
    fn test_quit_prompt_uppercase_y() {
        let result = handle_quit_prompt(KeyEvent::new(KeyCode::Char('Y'), KeyModifiers::NONE));
        assert!(matches!(result, QuitPromptResult::SaveAndExit));
    }

    #[test]
    fn test_quit_prompt_uppercase_n() {
        let result = handle_quit_prompt(KeyEvent::new(KeyCode::Char('N'), KeyModifiers::NONE));
        assert!(matches!(result, QuitPromptResult::Exit));
    }

    #[test]
    fn test_quit_prompt_russian_yes() {
        // Russian 'н' (QWERTY Y key position)
        let result = handle_quit_prompt(KeyEvent::new(KeyCode::Char('н'), KeyModifiers::NONE));
        assert!(matches!(result, QuitPromptResult::SaveAndExit));
    }

    #[test]
    fn test_quit_prompt_russian_no() {
        // Russian 'т' (QWERTY N key position)
        let result = handle_quit_prompt(KeyEvent::new(KeyCode::Char('т'), KeyModifiers::NONE));
        assert!(matches!(result, QuitPromptResult::Exit));
    }

    #[test]
    fn test_quit_prompt_russian_cancel() {
        // Russian 'с' (QWERTY C key position)
        let result = handle_quit_prompt(KeyEvent::new(KeyCode::Char('с'), KeyModifiers::NONE));
        assert!(matches!(result, QuitPromptResult::Cancel));
    }

    #[test]
    fn test_quit_prompt_c_cancels() {
        let result = handle_quit_prompt(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE));
        assert!(matches!(result, QuitPromptResult::Cancel));
    }

    #[test]
    fn test_quit_prompt_uppercase_c_cancels() {
        let result = handle_quit_prompt(KeyEvent::new(KeyCode::Char('C'), KeyModifiers::NONE));
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

    #[test]
    fn test_annotation_input_cyrillic_char() {
        // Test inserting Cyrillic character (multi-byte UTF-8)
        let lines = vec![
            Line { content: "line1".to_string(), annotation: None },
        ];
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
        ).unwrap();

        assert!(matches!(result, AnnotationModeResult::Continue));
        assert_eq!(buffer, "Helloв");
        assert_eq!(cursor_pos, 6); // Character count, not byte count
    }

    #[test]
    fn test_annotation_input_emoji() {
        // Test inserting emoji (4 bytes in UTF-8)
        let lines = vec![
            Line { content: "line1".to_string(), annotation: None },
        ];
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
        ).unwrap();

        assert!(matches!(result, AnnotationModeResult::Continue));
        assert_eq!(buffer, "Test🎉");
        assert_eq!(cursor_pos, 5); // 5 characters total
    }

    #[test]
    fn test_annotation_input_backspace_cyrillic() {
        // Test backspace with Cyrillic character
        let lines = vec![
            Line { content: "line1".to_string(), annotation: None },
        ];
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
        ).unwrap();

        assert!(matches!(result, AnnotationModeResult::Continue));
        assert_eq!(buffer, "Hello");
        assert_eq!(cursor_pos, 5);
    }

    #[test]
    fn test_annotation_input_mixed_multibyte() {
        // Test inserting multiple multi-byte characters in sequence
        let lines = vec![
            Line { content: "line1".to_string(), annotation: None },
        ];
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
            ).unwrap();
        }

        assert_eq!(buffer, "Привет");
        assert_eq!(cursor_pos, 6); // 6 characters
        assert_eq!(buffer.len(), 12); // 12 bytes (each Cyrillic char is 2 bytes)
    }

    #[test]
    fn test_annotation_input_right_arrow_with_cyrillic() {
        // Test Right arrow key with Cyrillic text
        let lines = vec![
            Line { content: "line1".to_string(), annotation: None },
        ];
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
        ).unwrap();

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
        ).unwrap();

        assert_eq!(cursor_pos, 6); // Should still be at end
    }

    #[test]
    fn test_annotation_input_insert_middle_cyrillic() {
        // Test inserting character in middle of Cyrillic text
        let lines = vec![
            Line { content: "line1".to_string(), annotation: None },
        ];
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
        ).unwrap();

        assert!(matches!(result, AnnotationModeResult::Continue));
        assert_eq!(buffer, "Прии");
        assert_eq!(cursor_pos, 3);
    }

    // ========================================================================
    // Alt+Left/Alt+Right Word Navigation Tests
    // ========================================================================

    #[test]
    fn test_annotation_input_alt_right_basic() {
        let lines = vec![
            Line { content: "line1".to_string(), annotation: None },
        ];
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
        ).unwrap();

        assert!(matches!(result, AnnotationModeResult::Continue));
        assert_eq!(cursor_pos, 6); // Jump to "world"
    }

    #[test]
    fn test_annotation_input_alt_left_basic() {
        let lines = vec![
            Line { content: "line1".to_string(), annotation: None },
        ];
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
        ).unwrap();

        assert!(matches!(result, AnnotationModeResult::Continue));
        assert_eq!(cursor_pos, 12); // Jump to "foo"
    }

    #[test]
    fn test_annotation_input_word_nav_cyrillic() {
        let lines = vec![
            Line { content: "line1".to_string(), annotation: None },
        ];
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
        ).unwrap();

        assert_eq!(cursor_pos, 7); // After "Привет "

        // Continue to next word
        handle_annotation_input(
            KeyEvent::new(KeyCode::Right, KeyModifiers::ALT),
            &mut buffer,
            &mut cursor_pos,
            &lines,
            0,
            &mut annotation_scroll,
        ).unwrap();

        assert_eq!(cursor_pos, 11); // After "мир "

        // Alt+Left to go back
        handle_annotation_input(
            KeyEvent::new(KeyCode::Left, KeyModifiers::ALT),
            &mut buffer,
            &mut cursor_pos,
            &lines,
            0,
            &mut annotation_scroll,
        ).unwrap();

        assert_eq!(cursor_pos, 7); // Back to "мир"
    }

    #[test]
    fn test_annotation_input_word_nav_with_punctuation() {
        let lines = vec![
            Line { content: "line1".to_string(), annotation: None },
        ];
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
        ).unwrap();

        assert_eq!(cursor_pos, 6); // Jump to "fix" (skip "TODO:" and space)

        handle_annotation_input(
            KeyEvent::new(KeyCode::Right, KeyModifiers::ALT),
            &mut buffer,
            &mut cursor_pos,
            &lines,
            0,
            &mut annotation_scroll,
        ).unwrap();

        assert_eq!(cursor_pos, 10); // Jump to "bug" (skip space)
    }

    #[test]
    fn test_annotation_input_word_nav_mixed_unicode() {
        let lines = vec![
            Line { content: "line1".to_string(), annotation: None },
        ];
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
        ).unwrap();

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
            ).unwrap();
            count += 1;
        }

        // Should reach end without panic
        assert_eq!(cursor_pos, buffer.chars().count());
    }

    #[test]
    fn test_annotation_input_word_nav_emoji() {
        let lines = vec![
            Line { content: "line1".to_string(), annotation: None },
        ];
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
        ).unwrap();

        assert_eq!(cursor_pos, 5); // After "Done "

        handle_annotation_input(
            KeyEvent::new(KeyCode::Right, KeyModifiers::ALT),
            &mut buffer,
            &mut cursor_pos,
            &lines,
            0,
            &mut annotation_scroll,
        ).unwrap();

        assert_eq!(cursor_pos, 7); // After "🎉 "
    }

    #[test]
    fn test_annotation_input_word_nav_at_boundaries() {
        let lines = vec![
            Line { content: "line1".to_string(), annotation: None },
        ];
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
        ).unwrap();

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
        ).unwrap();

        assert_eq!(cursor_pos, buffer.chars().count());
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

    // ========================================================================
    // Tree Input Handler Tests
    // ========================================================================

    #[test]
    fn test_tree_input_navigate_up() {
        use tempfile::TempDir;
        use std::fs::File;

        let dir = TempDir::new().unwrap();
        File::create(dir.path().join("file_a.txt")).unwrap();
        File::create(dir.path().join("file_b.txt")).unwrap();

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();
        panel.selected_index = 1; // Start on second file

        let result = handle_tree_input(
            KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
            &mut panel,
            30,
        ).unwrap();

        assert!(matches!(result, TreeInputResult::Continue));
        assert_eq!(panel.selected_index, 0);
    }

    #[test]
    fn test_tree_input_navigate_down() {
        use tempfile::TempDir;
        use std::fs::File;

        let dir = TempDir::new().unwrap();
        File::create(dir.path().join("file_a.txt")).unwrap();
        File::create(dir.path().join("file_b.txt")).unwrap();

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();
        panel.selected_index = 0;

        let result = handle_tree_input(
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            &mut panel,
            30,
        ).unwrap();

        assert!(matches!(result, TreeInputResult::Continue));
        assert_eq!(panel.selected_index, 1);
    }

    #[test]
    fn test_tree_input_navigate_home() {
        use tempfile::TempDir;
        use std::fs::File;

        let dir = TempDir::new().unwrap();
        for i in 0..5 {
            File::create(dir.path().join(format!("file_{}.txt", i))).unwrap();
        }

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();
        panel.selected_index = 3;

        let result = handle_tree_input(
            KeyEvent::new(KeyCode::Home, KeyModifiers::NONE),
            &mut panel,
            30,
        ).unwrap();

        assert!(matches!(result, TreeInputResult::Continue));
        assert_eq!(panel.selected_index, 0);
    }

    #[test]
    fn test_tree_input_navigate_end() {
        use tempfile::TempDir;
        use std::fs::File;

        let dir = TempDir::new().unwrap();
        for i in 0..5 {
            File::create(dir.path().join(format!("file_{}.txt", i))).unwrap();
        }

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();
        panel.selected_index = 0;

        let result = handle_tree_input(
            KeyEvent::new(KeyCode::End, KeyModifiers::NONE),
            &mut panel,
            30,
        ).unwrap();

        assert!(matches!(result, TreeInputResult::Continue));
        assert_eq!(panel.selected_index, panel.entries.len() - 1);
    }

    #[test]
    fn test_tree_input_page_up() {
        use tempfile::TempDir;
        use std::fs::File;

        let dir = TempDir::new().unwrap();
        for i in 0..20 {
            File::create(dir.path().join(format!("file_{:02}.txt", i))).unwrap();
        }

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();
        panel.selected_index = 15;

        // Terminal height 30, page_size = 30 - 8 = 22
        let result = handle_tree_input(
            KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE),
            &mut panel,
            30,
        ).unwrap();

        assert!(matches!(result, TreeInputResult::Continue));
        assert!(panel.selected_index < 15);
    }

    #[test]
    fn test_tree_input_page_down() {
        use tempfile::TempDir;
        use std::fs::File;

        let dir = TempDir::new().unwrap();
        for i in 0..20 {
            File::create(dir.path().join(format!("file_{:02}.txt", i))).unwrap();
        }

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();
        panel.selected_index = 5;

        let result = handle_tree_input(
            KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE),
            &mut panel,
            30,
        ).unwrap();

        assert!(matches!(result, TreeInputResult::Continue));
        assert!(panel.selected_index > 5);
    }

    #[test]
    fn test_tree_input_expand_directory() {
        use tempfile::TempDir;
        use std::fs::{self, File};

        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("subdir")).unwrap();
        File::create(dir.path().join("subdir/nested.txt")).unwrap();
        File::create(dir.path().join("file.txt")).unwrap();

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        // Find the subdir entry and select it
        let subdir_idx = panel.entries.iter()
            .position(|e| e.name == "subdir")
            .unwrap();
        panel.selected_index = subdir_idx;

        // Press Right to expand
        let result = handle_tree_input(
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            &mut panel,
            30,
        ).unwrap();

        assert!(matches!(result, TreeInputResult::Continue));

        // After expanding, nested.txt should be visible
        let has_nested = panel.entries.iter().any(|e| e.name == "nested.txt");
        assert!(has_nested, "Nested file should be visible after expand");
    }

    #[test]
    fn test_tree_input_collapse_directory() {
        use tempfile::TempDir;
        use std::fs::{self, File};

        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("subdir")).unwrap();
        File::create(dir.path().join("subdir/nested.txt")).unwrap();

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        // Find and expand subdir first
        let subdir_idx = panel.entries.iter()
            .position(|e| e.name == "subdir")
            .unwrap();
        panel.selected_index = subdir_idx;
        panel.expand_selected().unwrap();

        // Now collapse with Left key
        let result = handle_tree_input(
            KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
            &mut panel,
            30,
        ).unwrap();

        assert!(matches!(result, TreeInputResult::Continue));

        // After collapsing, nested.txt should not be visible
        let has_nested = panel.entries.iter().any(|e| e.name == "nested.txt");
        assert!(!has_nested, "Nested file should not be visible after collapse");
    }

    #[test]
    fn test_tree_input_enter_opens_file() {
        use tempfile::TempDir;
        use std::fs::File;

        let dir = TempDir::new().unwrap();
        File::create(dir.path().join("file.txt")).unwrap();

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        // Find the file entry
        let file_idx = panel.entries.iter()
            .position(|e| e.name == "file.txt")
            .unwrap();
        panel.selected_index = file_idx;

        let result = handle_tree_input(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut panel,
            30,
        ).unwrap();

        assert!(matches!(result, TreeInputResult::OpenFile(_)));
        if let TreeInputResult::OpenFile(path) = result {
            assert!(path.ends_with("file.txt"));
        }
    }

    #[test]
    fn test_tree_input_enter_toggles_directory() {
        use tempfile::TempDir;
        use std::fs::{self, File};

        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("subdir")).unwrap();
        File::create(dir.path().join("subdir/nested.txt")).unwrap();

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        // Find the subdir entry
        let subdir_idx = panel.entries.iter()
            .position(|e| e.name == "subdir")
            .unwrap();
        panel.selected_index = subdir_idx;

        // Press Enter to expand
        let result = handle_tree_input(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut panel,
            30,
        ).unwrap();

        assert!(matches!(result, TreeInputResult::Continue));
        let has_nested = panel.entries.iter().any(|e| e.name == "nested.txt");
        assert!(has_nested, "Should expand on Enter");

        // Press Enter again to collapse
        panel.selected_index = panel.entries.iter()
            .position(|e| e.name == "subdir")
            .unwrap();
        let result = handle_tree_input(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut panel,
            30,
        ).unwrap();

        assert!(matches!(result, TreeInputResult::Continue));
        let has_nested = panel.entries.iter().any(|e| e.name == "nested.txt");
        assert!(!has_nested, "Should collapse on second Enter");
    }

    #[test]
    fn test_tree_input_unhandled_key_continues() {
        use tempfile::TempDir;
        use std::fs::File;

        let dir = TempDir::new().unwrap();
        File::create(dir.path().join("file.txt")).unwrap();

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        // Press an unhandled key
        let result = handle_tree_input(
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
            &mut panel,
            30,
        ).unwrap();

        assert!(matches!(result, TreeInputResult::Continue));
    }

    #[test]
    fn test_tree_input_navigate_up_at_boundary() {
        use tempfile::TempDir;
        use std::fs::File;

        let dir = TempDir::new().unwrap();
        File::create(dir.path().join("file.txt")).unwrap();

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();
        panel.selected_index = 0; // Already at top

        let result = handle_tree_input(
            KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
            &mut panel,
            30,
        ).unwrap();

        assert!(matches!(result, TreeInputResult::Continue));
        assert_eq!(panel.selected_index, 0); // Should stay at 0
    }

    #[test]
    fn test_tree_input_navigate_down_at_boundary() {
        use tempfile::TempDir;
        use std::fs::File;

        let dir = TempDir::new().unwrap();
        File::create(dir.path().join("file.txt")).unwrap();

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();
        let last_idx = panel.entries.len() - 1;
        panel.selected_index = last_idx;

        let result = handle_tree_input(
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            &mut panel,
            30,
        ).unwrap();

        assert!(matches!(result, TreeInputResult::Continue));
        assert_eq!(panel.selected_index, last_idx); // Should stay at last
    }

    // ========================================================================
    // Edge Cases for Cursor Positions
    // ========================================================================

    #[test]
    fn test_cursor_position_after_delete_annotation() {
        let mut lines = vec![
            Line { content: "line1".to_string(), annotation: Some("test".to_string()) },
            Line { content: "line2".to_string(), annotation: None },
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
        ).unwrap();

        assert!(matches!(result, IdleModeResult::Action(_)));
        assert_eq!(cursor_line, 0); // Cursor should stay on same line
    }

    #[test]
    fn test_cursor_position_after_navigation() {
        let mut lines = vec![
            Line { content: "line1".to_string(), annotation: Some("a1".to_string()) },
            Line { content: "line2".to_string(), annotation: None },
            Line { content: "line3".to_string(), annotation: Some("a3".to_string()) },
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
        ).unwrap();

        assert_eq!(cursor_line, 2); // Should jump to line 3 (index 2)
    }

    #[test]
    fn test_cursor_position_at_file_boundaries() {
        let mut lines = vec![
            Line { content: "only line".to_string(), annotation: None },
        ];
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
        ).unwrap();

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
        ).unwrap();

        assert_eq!(cursor_line, 0); // Should stay at 0
    }

    #[test]
    fn test_annotation_cursor_position_empty_buffer() {
        let lines = vec![
            Line { content: "line1".to_string(), annotation: None },
        ];
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
        ).unwrap();

        assert!(matches!(result, AnnotationModeResult::Continue));
        assert_eq!(cursor_pos, 0);
        assert_eq!(buffer, "");
    }

    #[test]
    fn test_annotation_cursor_position_backspace_at_start() {
        let lines = vec![
            Line { content: "line1".to_string(), annotation: None },
        ];
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
        ).unwrap();

        assert!(matches!(result, AnnotationModeResult::Continue));
        assert_eq!(buffer, "test"); // Buffer unchanged
        assert_eq!(cursor_pos, 0); // Cursor stays at 0
    }

    #[test]
    fn test_annotation_cursor_movement_home_end() {
        let lines = vec![
            Line { content: "line1".to_string(), annotation: None },
        ];
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
        ).unwrap();
        assert_eq!(cursor_pos, 0);

        // Press End
        handle_annotation_input(
            KeyEvent::new(KeyCode::End, KeyModifiers::NONE),
            &mut buffer,
            &mut cursor_pos,
            &lines,
            0,
            &mut annotation_scroll,
        ).unwrap();
        assert_eq!(cursor_pos, buffer.chars().count());
    }

    #[test]
    fn test_annotation_cursor_left_right() {
        let lines = vec![
            Line { content: "line1".to_string(), annotation: None },
        ];
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
        ).unwrap();
        assert_eq!(cursor_pos, 1);

        // Press Right
        handle_annotation_input(
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            &mut buffer,
            &mut cursor_pos,
            &lines,
            0,
            &mut annotation_scroll,
        ).unwrap();
        assert_eq!(cursor_pos, 2);
    }

    #[test]
    fn test_search_cursor_position_updates() {
        let lines = vec![
            Line { content: "first match here".to_string(), annotation: None },
            Line { content: "no match".to_string(), annotation: None },
            Line { content: "second match here".to_string(), annotation: None },
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
        ).unwrap();

        assert_eq!(current_match, Some(1));
        assert_eq!(cursor_line, 2); // Should move to line 2
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
        ).unwrap();

        assert!(matches!(result, IdleModeResult::Continue));
        // Cursor should remain valid (0)
        assert_eq!(cursor_line, 0);
    }
}
