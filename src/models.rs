use crate::diff::DiffResult;

/// Which panel currently has focus
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum FocusedPanel {
    /// Editor panel has focus
    #[default]
    Editor,
    /// File tree panel has focus
    FileTree,
}

#[derive(Clone)]
pub struct Line {
    pub content: String,
    pub annotation: Option<String>,
}

// ============================================================================
// New Architecture: ViewMode + EditorState (orthogonal concerns)
// ============================================================================

/// How the main content area is rendered.
/// This affects ONLY the visual presentation, not input handling.
#[derive(Clone)]
pub enum ViewMode {
    /// Standard single-pane view
    Normal,
    /// Split-pane diff view comparing working copy to HEAD
    Diff { diff_result: DiffResult },
}

impl Default for ViewMode {
    fn default() -> Self {
        ViewMode::Normal
    }
}

/// What input mode the user is in.
/// This affects ONLY input handling, independent of view mode.
pub enum EditorState {
    /// Normal navigation, all shortcuts active
    Idle,
    /// Editing an annotation for the current line
    Annotating { buffer: String, cursor_pos: usize },
    /// Searching for text in the file
    Searching { query: String, cursor_pos: usize },
    /// Showing help overlay
    ShowingHelp,
    /// Asking about unsaved changes before quit
    QuitPrompt,
    /// Asking about unsaved changes before switching files
    FileSwitchPrompt { pending_path: std::path::PathBuf },
}

impl Default for EditorState {
    fn default() -> Self {
        EditorState::Idle
    }
}

#[derive(Clone, Debug)]
pub enum Action {
    EditAnnotation {
        line_index: usize,
        old_text: Option<String>,
        new_text: Option<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::{DiffLine, DiffResult, LineChange};

    // =========================================================================
    // ViewMode Tests
    // =========================================================================

    #[test]
    fn test_view_mode_normal_is_default() {
        let view_mode = ViewMode::default();
        assert!(matches!(view_mode, ViewMode::Normal));
    }

    #[test]
    fn test_view_mode_diff_holds_diff_result() {
        let diff_result = DiffResult {
            lines: vec![DiffLine {
                working: Some((1, "line1".to_string(), LineChange::Unchanged)),
                head: Some((1, "line1".to_string(), LineChange::Unchanged)),
            }],
        };
        let view_mode = ViewMode::Diff { diff_result: diff_result.clone() };

        if let ViewMode::Diff { diff_result: stored } = view_mode {
            assert_eq!(stored.lines.len(), 1);
        } else {
            panic!("Expected ViewMode::Diff");
        }
    }

    #[test]
    fn test_view_mode_clone() {
        let diff_result = DiffResult {
            lines: vec![DiffLine {
                working: Some((1, "test".to_string(), LineChange::Added)),
                head: None,
            }],
        };
        let original = ViewMode::Diff { diff_result };
        let cloned = original.clone();

        if let (ViewMode::Diff { diff_result: d1 }, ViewMode::Diff { diff_result: d2 }) = (&original, &cloned) {
            assert_eq!(d1.lines.len(), d2.lines.len());
        } else {
            panic!("Clone failed");
        }
    }

    // =========================================================================
    // EditorState Tests
    // =========================================================================

    #[test]
    fn test_editor_state_idle_is_default() {
        let state = EditorState::default();
        assert!(matches!(state, EditorState::Idle));
    }

    #[test]
    fn test_editor_state_annotating_holds_buffer() {
        let state = EditorState::Annotating {
            buffer: "test annotation".to_string(),
            cursor_pos: 5,
        };

        if let EditorState::Annotating { buffer, cursor_pos } = state {
            assert_eq!(buffer, "test annotation");
            assert_eq!(cursor_pos, 5);
        } else {
            panic!("Expected EditorState::Annotating");
        }
    }

    #[test]
    fn test_editor_state_searching_holds_query() {
        let state = EditorState::Searching {
            query: "search term".to_string(),
            cursor_pos: 11,
        };

        if let EditorState::Searching { query, cursor_pos } = state {
            assert_eq!(query, "search term");
            assert_eq!(cursor_pos, 11);
        } else {
            panic!("Expected EditorState::Searching");
        }
    }

    #[test]
    fn test_editor_state_showing_help() {
        let state = EditorState::ShowingHelp;
        assert!(matches!(state, EditorState::ShowingHelp));
    }

    #[test]
    fn test_editor_state_quit_prompt() {
        let state = EditorState::QuitPrompt;
        assert!(matches!(state, EditorState::QuitPrompt));
    }

    // =========================================================================
    // FocusedPanel Tests
    // =========================================================================

    #[test]
    fn test_focused_panel_default_is_editor() {
        let panel = FocusedPanel::default();
        assert_eq!(panel, FocusedPanel::Editor);
    }

    #[test]
    fn test_focused_panel_clone() {
        let panel = FocusedPanel::FileTree;
        let cloned = panel;
        assert_eq!(cloned, FocusedPanel::FileTree);
    }

    #[test]
    fn test_focused_panel_equality() {
        assert_eq!(FocusedPanel::Editor, FocusedPanel::Editor);
        assert_eq!(FocusedPanel::FileTree, FocusedPanel::FileTree);
        assert_ne!(FocusedPanel::Editor, FocusedPanel::FileTree);
    }

    #[test]
    fn test_focused_panel_copy_trait() {
        // Verify FocusedPanel implements Copy
        let panel = FocusedPanel::Editor;
        let copied = panel; // Should copy, not move
        // Both should still be usable
        assert_eq!(panel, FocusedPanel::Editor);
        assert_eq!(copied, FocusedPanel::Editor);
    }

    #[test]
    fn test_focused_panel_debug_format() {
        // Verify Debug trait formatting
        let editor_panel = FocusedPanel::Editor;
        let tree_panel = FocusedPanel::FileTree;
        let editor_debug = format!("{:?}", editor_panel);
        let tree_debug = format!("{:?}", tree_panel);

        assert!(editor_debug.contains("Editor"));
        assert!(tree_debug.contains("FileTree"));
    }
}
