//! Event handler module for keyboard input processing.
//!
//! This module handles keyboard events for different editor states:
//! - Idle mode: Main editing state with navigation and commands
//! - Annotation mode: Editing annotations on lines
//! - Search mode: Searching for text in the document
//! - Quit prompt: Confirming exit with unsaved changes
//! - Tree panel: File tree navigation

#![allow(clippy::too_many_arguments)]

mod annotation;
mod idle;
mod quit;
mod search;
mod tree;

pub use annotation::{handle_annotation_input, AnnotationModeResult};
pub use idle::{handle_idle_mode, IdleModeResult};
pub use quit::{handle_quit_prompt, QuitPromptResult};
pub use search::{handle_search_input, SearchModeResult};
pub use tree::{handle_tree_input, TreeInputResult};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

// ============================================================================
// Multi-Hotkey Helper for Keyboard Layout Independence
// ============================================================================

/// Check if a key event matches Ctrl+<one of the alternatives>.
/// This enables hotkeys to work across different keyboard layouts (EN/RU/CN/etc.).
///
/// For example, matches_ctrl_key(&key, &['x', 'ч']) will return true if:
/// - English layout: Ctrl+X pressed (produces 'x')
/// - Russian layout: Ctrl+X physical key pressed (produces 'ч')
pub(crate) fn matches_ctrl_key(key: &KeyEvent, alternatives: &[char]) -> bool {
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
pub(crate) fn matches_char(key_code: &KeyCode, alternatives: &[char]) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
