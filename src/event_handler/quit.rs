//! Quit prompt event handler.
//!
//! Handles keyboard events when showing the quit confirmation prompt.

use super::matches_char;
use crossterm::event::{KeyCode, KeyEvent};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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
}
