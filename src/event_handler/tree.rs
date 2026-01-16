//! File tree panel event handler.
//!
//! Handles keyboard events when the file tree panel is focused.

use crate::file_tree::FileTreePanel;
use crossterm::event::{KeyCode, KeyEvent};
use std::io;
use std::path::PathBuf;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_tree::FileTreePanel;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::fs::{self, File};
    use tempfile::TempDir;

    #[test]
    fn test_tree_input_navigate_up() {
        let dir = TempDir::new().unwrap();
        File::create(dir.path().join("file_a.txt")).unwrap();
        File::create(dir.path().join("file_b.txt")).unwrap();

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();
        panel.selected_index = 1; // Start on second file

        let result = handle_tree_input(
            KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
            &mut panel,
            30,
        )
        .unwrap();

        assert!(matches!(result, TreeInputResult::Continue));
        assert_eq!(panel.selected_index, 0);
    }

    #[test]
    fn test_tree_input_navigate_down() {
        let dir = TempDir::new().unwrap();
        File::create(dir.path().join("file_a.txt")).unwrap();
        File::create(dir.path().join("file_b.txt")).unwrap();

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();
        panel.selected_index = 0;

        let result = handle_tree_input(
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            &mut panel,
            30,
        )
        .unwrap();

        assert!(matches!(result, TreeInputResult::Continue));
        assert_eq!(panel.selected_index, 1);
    }

    #[test]
    fn test_tree_input_navigate_home() {
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
        )
        .unwrap();

        assert!(matches!(result, TreeInputResult::Continue));
        assert_eq!(panel.selected_index, 0);
    }

    #[test]
    fn test_tree_input_navigate_end() {
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
        )
        .unwrap();

        assert!(matches!(result, TreeInputResult::Continue));
        assert_eq!(panel.selected_index, panel.entries.len() - 1);
    }

    #[test]
    fn test_tree_input_page_up() {
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
        )
        .unwrap();

        assert!(matches!(result, TreeInputResult::Continue));
        assert!(panel.selected_index < 15);
    }

    #[test]
    fn test_tree_input_page_down() {
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
        )
        .unwrap();

        assert!(matches!(result, TreeInputResult::Continue));
        assert!(panel.selected_index > 5);
    }

    #[test]
    fn test_tree_input_expand_directory() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("subdir")).unwrap();
        File::create(dir.path().join("subdir/nested.txt")).unwrap();
        File::create(dir.path().join("file.txt")).unwrap();

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        // Find the subdir entry and select it
        let subdir_idx = panel
            .entries
            .iter()
            .position(|e| e.name == "subdir")
            .unwrap();
        panel.selected_index = subdir_idx;

        // Press Right to expand
        let result = handle_tree_input(
            KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
            &mut panel,
            30,
        )
        .unwrap();

        assert!(matches!(result, TreeInputResult::Continue));

        // After expanding, nested.txt should be visible
        let has_nested = panel.entries.iter().any(|e| e.name == "nested.txt");
        assert!(has_nested, "Nested file should be visible after expand");
    }

    #[test]
    fn test_tree_input_collapse_directory() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("subdir")).unwrap();
        File::create(dir.path().join("subdir/nested.txt")).unwrap();

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        // Find and expand subdir first
        let subdir_idx = panel
            .entries
            .iter()
            .position(|e| e.name == "subdir")
            .unwrap();
        panel.selected_index = subdir_idx;
        panel.expand_selected().unwrap();

        // Now collapse with Left key
        let result = handle_tree_input(
            KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
            &mut panel,
            30,
        )
        .unwrap();

        assert!(matches!(result, TreeInputResult::Continue));

        // After collapsing, nested.txt should not be visible
        let has_nested = panel.entries.iter().any(|e| e.name == "nested.txt");
        assert!(
            !has_nested,
            "Nested file should not be visible after collapse"
        );
    }

    #[test]
    fn test_tree_input_enter_opens_file() {
        let dir = TempDir::new().unwrap();
        File::create(dir.path().join("file.txt")).unwrap();

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        // Find the file entry
        let file_idx = panel
            .entries
            .iter()
            .position(|e| e.name == "file.txt")
            .unwrap();
        panel.selected_index = file_idx;

        let result = handle_tree_input(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut panel,
            30,
        )
        .unwrap();

        assert!(matches!(result, TreeInputResult::OpenFile(_)));
        if let TreeInputResult::OpenFile(path) = result {
            assert!(path.ends_with("file.txt"));
        }
    }

    #[test]
    fn test_tree_input_enter_toggles_directory() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("subdir")).unwrap();
        File::create(dir.path().join("subdir/nested.txt")).unwrap();

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        // Find the subdir entry
        let subdir_idx = panel
            .entries
            .iter()
            .position(|e| e.name == "subdir")
            .unwrap();
        panel.selected_index = subdir_idx;

        // Press Enter to expand
        let result = handle_tree_input(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut panel,
            30,
        )
        .unwrap();

        assert!(matches!(result, TreeInputResult::Continue));
        let has_nested = panel.entries.iter().any(|e| e.name == "nested.txt");
        assert!(has_nested, "Should expand on Enter");

        // Press Enter again to collapse
        panel.selected_index = panel
            .entries
            .iter()
            .position(|e| e.name == "subdir")
            .unwrap();
        let result = handle_tree_input(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut panel,
            30,
        )
        .unwrap();

        assert!(matches!(result, TreeInputResult::Continue));
        let has_nested = panel.entries.iter().any(|e| e.name == "nested.txt");
        assert!(!has_nested, "Should collapse on second Enter");
    }

    #[test]
    fn test_tree_input_unhandled_key_continues() {
        let dir = TempDir::new().unwrap();
        File::create(dir.path().join("file.txt")).unwrap();

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();

        // Press an unhandled key
        let result = handle_tree_input(
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
            &mut panel,
            30,
        )
        .unwrap();

        assert!(matches!(result, TreeInputResult::Continue));
    }

    #[test]
    fn test_tree_input_navigate_up_at_boundary() {
        let dir = TempDir::new().unwrap();
        File::create(dir.path().join("file.txt")).unwrap();

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();
        panel.selected_index = 0; // Already at top

        let result = handle_tree_input(
            KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
            &mut panel,
            30,
        )
        .unwrap();

        assert!(matches!(result, TreeInputResult::Continue));
        assert_eq!(panel.selected_index, 0); // Should stay at 0
    }

    #[test]
    fn test_tree_input_navigate_down_at_boundary() {
        let dir = TempDir::new().unwrap();
        File::create(dir.path().join("file.txt")).unwrap();

        let mut panel = FileTreePanel::new(dir.path().to_path_buf()).unwrap();
        let last_idx = panel.entries.len() - 1;
        panel.selected_index = last_idx;

        let result = handle_tree_input(
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            &mut panel,
            30,
        )
        .unwrap();

        assert!(matches!(result, TreeInputResult::Continue));
        assert_eq!(panel.selected_index, last_idx); // Should stay at last
    }
}
