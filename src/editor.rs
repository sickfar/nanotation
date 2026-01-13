use crate::diff::{calculate_diff, strip_annotation};
use crate::event_handler;
use crate::file;
use crate::git;
use crate::models::{EditorState, Line, ViewMode};
use crate::theme::Theme;
use crate::ui;
use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event, KeyCode},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io;

/// Minimum terminal width required for diff mode (100 columns)
const MIN_DIFF_WIDTH: u16 = 100;

/// Error message when terminal is too narrow for diff mode
pub const DIFF_WIDTH_ERROR: &str = "Terminal too narrow for diff view (min 100 columns)";

/// Error message when file is not tracked in git
pub const DIFF_NOT_TRACKED_ERROR: &str = "File is not tracked in git";

/// Error message when file is not in a git repository
pub const DIFF_NO_REPO_ERROR: &str = "Not a git repository";

pub struct Editor {
    pub lines: Vec<Line>,
    pub cursor_line: usize,
    pub scroll_offset: usize,
    /// How the main content area is rendered (Normal or Diff)
    pub view_mode: ViewMode,
    /// What input mode the user is in (Idle, Annotating, etc.)
    pub editor_state: EditorState,
    pub file_path: Option<String>,
    /// Hash of content at last save (for detecting unsaved changes)
    saved_content_hash: u64,
    pub theme: Theme,
    pub lang_comment: String,
    pub search_matches: Vec<usize>,
    pub current_match: Option<usize>,
    pub annotation_scroll: usize,
    pub history: Vec<crate::models::Action>,
    pub history_index: usize,
    pub highlighter: crate::highlighting::SyntaxHighlighter,
    /// Error message to display in status bar (clears on next action)
    pub status_message: Option<String>,
}

impl Editor {
    pub fn new(file_path: String) -> io::Result<Self> {
        let content = fs::read_to_string(&file_path)?;
        let lang_comment = file::detect_comment_style(&file_path);
        let lines = file::parse_file(&content, &lang_comment);
        let theme = Theme::Dark;
        let highlighter =
            crate::highlighting::SyntaxHighlighter::new(matches!(theme, Theme::Dark));
        let saved_content_hash = Self::compute_content_hash(&lines);

        Ok(Editor {
            lines,
            cursor_line: 0,
            scroll_offset: 0,
            view_mode: ViewMode::Normal,
            editor_state: EditorState::Idle,
            file_path: Some(file_path),
            saved_content_hash,
            theme,
            lang_comment,
            search_matches: Vec::new(),
            current_match: None,
            annotation_scroll: 0,
            history: Vec::new(),
            history_index: 0,
            highlighter,
            status_message: None,
        })
    }

    /// Compute a hash of the current content (lines + annotations)
    fn compute_content_hash(lines: &[Line]) -> u64 {
        let mut hasher = DefaultHasher::new();
        for line in lines {
            line.content.hash(&mut hasher);
            line.annotation.hash(&mut hasher);
        }
        hasher.finish()
    }

    /// Check if content has been modified since last save
    pub fn is_modified(&self) -> bool {
        Self::compute_content_hash(&self.lines) != self.saved_content_hash
    }

    /// Try to enter diff mode. Returns error message if not possible.
    /// Only changes view_mode, does not affect editor_state.
    pub fn enter_diff_mode(&mut self) -> Result<(), &'static str> {
        // Check if diff is available
        let file_path = self.file_path.as_ref().ok_or(DIFF_NO_REPO_ERROR)?;

        if !git::is_git_available(file_path) {
            return Err(DIFF_NO_REPO_ERROR);
        }

        if !git::is_file_tracked(file_path) {
            return Err(DIFF_NOT_TRACKED_ERROR);
        }

        // Check terminal width
        let (width, _) = terminal::size().map_err(|_| DIFF_WIDTH_ERROR)?;
        if width < MIN_DIFF_WIDTH {
            return Err(DIFF_WIDTH_ERROR);
        }

        // Get HEAD content
        let head_content = git::get_head_content(file_path).map_err(|e| match e {
            git::GitError::NotARepo => DIFF_NO_REPO_ERROR,
            git::GitError::NotTracked => DIFF_NOT_TRACKED_ERROR,
            git::GitError::NotInHead => DIFF_NOT_TRACKED_ERROR,
            git::GitError::Git(_) => DIFF_NO_REPO_ERROR,
        })?;

        // Calculate diff
        let diff_result = calculate_diff(&self.lines, &head_content, &self.lang_comment);

        // Only change view_mode, not editor_state
        self.view_mode = ViewMode::Diff { diff_result };
        Ok(())
    }

    /// Exit diff mode and return to normal view.
    /// Only changes view_mode, does not affect editor_state.
    pub fn exit_diff_mode(&mut self) {
        self.view_mode = ViewMode::Normal;
    }

    /// Toggle diff mode on/off.
    /// Only changes view_mode, does not affect editor_state.
    pub fn toggle_diff_mode(&mut self) {
        if matches!(self.view_mode, ViewMode::Diff { .. }) {
            self.exit_diff_mode();
        } else {
            match self.enter_diff_mode() {
                Ok(()) => {}
                Err(msg) => {
                    self.status_message = Some(msg.to_string());
                }
            }
        }
    }

    pub fn save(&mut self) -> io::Result<()> {
        if let Some(ref path) = self.file_path {
            file::save_file(path, &self.lines, &self.lang_comment)?;
            // Update hash to reflect saved state
            self.saved_content_hash = Self::compute_content_hash(&self.lines);
        }
        Ok(())
    }

    pub fn perform_action(&mut self, action: crate::models::Action) {
        // If we are not at the end of history, truncate
        if self.history_index < self.history.len() {
            self.history.truncate(self.history_index);
        }
        self.history.push(action);
        self.history_index += 1;
        // No need to set modified flag - is_modified() uses hash comparison
    }

    pub fn undo(&mut self) {
        if self.history_index > 0 {
            self.history_index -= 1;
            match &self.history[self.history_index] {
                crate::models::Action::EditAnnotation { line_index, old_text, .. } => {
                    self.lines[*line_index].annotation = old_text.clone();
                }
            }
            // No need to set modified flag - is_modified() uses hash comparison
        }
    }

    pub fn redo(&mut self) {
        if self.history_index < self.history.len() {
            match &self.history[self.history_index] {
                crate::models::Action::EditAnnotation { line_index, new_text, .. } => {
                    self.lines[*line_index].annotation = new_text.clone();
                }
            }
            self.history_index += 1;
            // No need to set modified flag - is_modified() uses hash comparison
        }
    }

    pub fn run(&mut self) -> io::Result<()> {
        terminal::enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen, Hide)?;

        let result = self.event_loop();

        execute!(io::stdout(), LeaveAlternateScreen, Show)?;
        terminal::disable_raw_mode()?;

        result
    }

    fn event_loop(&mut self) -> io::Result<()> {
        loop {
            // Check if diff is available (git repo + tracked file + has actual changes)
            let diff_available = self.file_path.as_ref().map_or(false, |path| {
                if !git::is_git_available(path) || !git::is_file_tracked(path) {
                    return false;
                }
                // Check if there are actual changes between working copy and HEAD
                if let Ok(head_content) = git::get_head_content(path) {
                    let working_content: String = self
                        .lines
                        .iter()
                        .map(|line| strip_annotation(&line.content, &self.lang_comment))
                        .collect::<Vec<_>>()
                        .join("\n");
                    let head_trimmed = head_content.trim_end();
                    working_content != head_trimmed
                } else {
                    false
                }
            });

            // Render using both view_mode and editor_state
            ui::render(
                &self.lines,
                self.cursor_line,
                self.scroll_offset,
                &self.view_mode,
                &self.editor_state,
                &self.file_path,
                self.is_modified(),
                self.theme,
                &self.search_matches,
                self.current_match,
                self.annotation_scroll,
                &self.highlighter,
                self.status_message.as_deref(),
                &self.lang_comment,
                diff_available,
            )?;

            // Clear status message after displaying
            self.status_message = None;

            if let Event::Key(key) = event::read()? {
                // Handle input based on editor_state (NOT view_mode)
                // view_mode only affects rendering, not input handling
                match &mut self.editor_state {
                    EditorState::Idle => {
                        // Handle save separately (Ctrl+O)
                        if key.code == KeyCode::Char('o')
                            && key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL)
                        {
                            self.save()?;
                            continue;
                        }

                        let current_theme_is_dark = matches!(self.theme, crate::theme::Theme::Dark);

                        match event_handler::handle_idle_mode(
                            key,
                            &mut self.lines,
                            &mut self.cursor_line,
                            &self.view_mode,
                            &mut self.theme,
                            &mut self.annotation_scroll,
                            &mut self.scroll_offset,
                        )? {
                            event_handler::IdleModeResult::Exit => break,
                            event_handler::IdleModeResult::ShowQuitPrompt => {
                                if self.is_modified() {
                                    self.editor_state = EditorState::QuitPrompt;
                                } else {
                                    break;
                                }
                            }
                            event_handler::IdleModeResult::Action(action) => {
                                // Apply and push history
                                match &action {
                                    crate::models::Action::EditAnnotation { line_index, new_text, .. } => {
                                        self.lines[*line_index].annotation = new_text.clone();
                                    }
                                }
                                self.perform_action(action);
                            }
                            event_handler::IdleModeResult::Undo => self.undo(),
                            event_handler::IdleModeResult::Redo => self.redo(),
                            event_handler::IdleModeResult::EnterAnnotation { initial_text } => {
                                let cursor_pos = initial_text.len();
                                self.editor_state = EditorState::Annotating {
                                    buffer: initial_text,
                                    cursor_pos,
                                };
                                // view_mode stays unchanged!
                            }
                            event_handler::IdleModeResult::EnterSearch => {
                                self.editor_state = EditorState::Searching {
                                    query: String::new(),
                                    cursor_pos: 0,
                                };
                                // view_mode stays unchanged!
                            }
                            event_handler::IdleModeResult::ShowHelp => {
                                self.editor_state = EditorState::ShowingHelp;
                                // view_mode stays unchanged!
                            }
                            event_handler::IdleModeResult::ToggleDiffView => {
                                self.toggle_diff_mode();
                                // editor_state stays Idle!
                            }
                            event_handler::IdleModeResult::ExitDiffView => {
                                self.view_mode = ViewMode::Normal;
                                // editor_state stays Idle!
                            }
                            event_handler::IdleModeResult::Continue => {
                                // Check if theme changed
                                let new_theme_is_dark = matches!(self.theme, crate::theme::Theme::Dark);
                                if current_theme_is_dark != new_theme_is_dark {
                                    self.highlighter = crate::highlighting::SyntaxHighlighter::new(new_theme_is_dark);
                                }
                            }
                        }
                    }

                    EditorState::Annotating { buffer, cursor_pos } => {
                        match event_handler::handle_annotation_input(
                            key,
                            buffer,
                            cursor_pos,
                            &self.lines,
                            self.cursor_line,
                            &mut self.annotation_scroll,
                        )? {
                            event_handler::AnnotationModeResult::Save(action) => {
                                // Apply and push history
                                match &action {
                                    crate::models::Action::EditAnnotation { line_index, new_text, .. } => {
                                        self.lines[*line_index].annotation = new_text.clone();
                                    }
                                }
                                self.perform_action(action);
                                self.editor_state = EditorState::Idle;
                                // view_mode stays unchanged!
                            }
                            event_handler::AnnotationModeResult::Cancel => {
                                self.editor_state = EditorState::Idle;
                                // view_mode stays unchanged!
                            }
                            event_handler::AnnotationModeResult::Continue => {
                                // Stay in annotating state
                            }
                        }
                    }

                    EditorState::Searching { query, cursor_pos } => {
                        match event_handler::handle_search_input(
                            key,
                            query,
                            cursor_pos,
                            &mut self.search_matches,
                            &mut self.current_match,
                            &self.lines,
                            &mut self.cursor_line,
                            &mut self.scroll_offset,
                            &self.view_mode,
                        )? {
                            event_handler::SearchModeResult::Exit => {
                                self.editor_state = EditorState::Idle;
                                // view_mode stays unchanged!
                            }
                            event_handler::SearchModeResult::Continue => {
                                // Stay in searching state
                            }
                        }
                    }

                    EditorState::ShowingHelp => {
                        // Any key exits help
                        self.editor_state = EditorState::Idle;
                        // view_mode stays unchanged!
                    }

                    EditorState::QuitPrompt => {
                        match event_handler::handle_quit_prompt(key) {
                            event_handler::QuitPromptResult::SaveAndExit => {
                                self.save()?;
                                break;
                            }
                            event_handler::QuitPromptResult::Exit => {
                                break;
                            }
                            event_handler::QuitPromptResult::Cancel => {
                                self.editor_state = EditorState::Idle;
                                // view_mode stays unchanged!
                            }
                            event_handler::QuitPromptResult::Continue => {
                                // Stay in quit prompt
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;



    #[test]

    fn test_search_functionality() {
        // Create dummy file for test
        let test_file = "test_search.txt";
        std::fs::write(test_file, "content").unwrap();
        
        let mut editor = Editor::new(test_file.to_string()).unwrap();
        std::fs::remove_file(test_file).unwrap(); // Cleanup
        
        editor.lines = vec![
            Line { content: "hello world".to_string(), annotation: None },
            Line { content: "foo bar".to_string(), annotation: None },
            Line { content: "hello again".to_string(), annotation: None },
        ];

        // Simulate search
        let query = "hello";
        editor.search_matches.clear();
        editor.current_match = None;

        let query_lower = query.to_lowercase();
        for (i, line) in editor.lines.iter().enumerate() {
            if line.content.to_lowercase().contains(&query_lower) {
                editor.search_matches.push(i);
            }
        }

        if !editor.search_matches.is_empty() {
            editor.current_match = Some(0);
            editor.cursor_line = editor.search_matches[0];
        }

        assert_eq!(editor.search_matches.len(), 2);
        assert_eq!(editor.search_matches[0], 0);
        assert_eq!(editor.search_matches[1], 2);
        assert_eq!(editor.current_match, Some(0));
        assert_eq!(editor.cursor_line, 0);
    }

    #[test]
    fn test_next_search_match_cycling() {
        let test_file = "test_cycle.txt";
        std::fs::write(test_file, "content").unwrap();
        let mut editor = Editor::new(test_file.to_string()).unwrap();
        std::fs::remove_file(test_file).unwrap();
        
        editor.search_matches = vec![0, 2, 4];
        editor.current_match = Some(0);
        editor.cursor_line = 0;

        // Next match
        if let Some(idx) = editor.current_match {
            if !editor.search_matches.is_empty() {
                let next = (idx + 1) % editor.search_matches.len();
                editor.current_match = Some(next);
                editor.cursor_line = editor.search_matches[next];
            }
        }

        assert_eq!(editor.current_match, Some(1));
        assert_eq!(editor.cursor_line, 2);

        // Next match again
        if let Some(idx) = editor.current_match {
            if !editor.search_matches.is_empty() {
                let next = (idx + 1) % editor.search_matches.len();
                editor.current_match = Some(next);
                editor.cursor_line = editor.search_matches[next];
            }
        }

        assert_eq!(editor.current_match, Some(2));
        assert_eq!(editor.cursor_line, 4);

        // Cycle back to first
        if let Some(idx) = editor.current_match {
            if !editor.search_matches.is_empty() {
                let next = (idx + 1) % editor.search_matches.len();
                editor.current_match = Some(next);
                editor.cursor_line = editor.search_matches[next];
            }
        }

        assert_eq!(editor.current_match, Some(0));
        assert_eq!(editor.cursor_line, 0);
    }

    #[test]
    fn test_undo_redo() {
        let test_file = "test_undo.txt";
        std::fs::write(test_file, "line1\nline2").unwrap();
        let mut editor = Editor::new(test_file.to_string()).unwrap();
        std::fs::remove_file(test_file).unwrap();

        // Initial state
        assert_eq!(editor.lines[0].annotation, None);
        // History initialized empty in new()

        // Perform action: Add annotation
        let action1 = crate::models::Action::EditAnnotation {
            line_index: 0,
            old_text: None,
            new_text: Some("note1".to_string()),
        };
        editor.lines[0].annotation = Some("note1".to_string());
        editor.perform_action(action1);

        assert_eq!(editor.lines[0].annotation, Some("note1".to_string()));
        assert_eq!(editor.history.len(), 1);
        assert_eq!(editor.history_index, 1);

        // Undo
        editor.undo();
        assert_eq!(editor.lines[0].annotation, None);
        assert_eq!(editor.history_index, 0);

        // Redo
        editor.redo();
        assert_eq!(editor.lines[0].annotation, Some("note1".to_string()));
        assert_eq!(editor.history_index, 1);
    }

    #[test]
    fn test_undo_at_beginning() {
        let test_file = "test_undo_begin.txt";
        std::fs::write(test_file, "line1").unwrap();
        let mut editor = Editor::new(test_file.to_string()).unwrap();
        std::fs::remove_file(test_file).unwrap();

        // No history yet
        assert_eq!(editor.history_index, 0);
        assert_eq!(editor.history.len(), 0);

        // Undo should have no effect
        editor.undo();
        assert_eq!(editor.history_index, 0);
    }

    #[test]
    fn test_redo_at_end() {
        let test_file = "test_redo_end.txt";
        std::fs::write(test_file, "line1").unwrap();
        let mut editor = Editor::new(test_file.to_string()).unwrap();
        std::fs::remove_file(test_file).unwrap();

        // Perform an action
        let action = crate::models::Action::EditAnnotation {
            line_index: 0,
            old_text: None,
            new_text: Some("note".to_string()),
        };
        editor.lines[0].annotation = Some("note".to_string());
        editor.perform_action(action);

        // Already at end of history
        assert_eq!(editor.history_index, 1);

        // Redo should have no effect
        editor.redo();
        assert_eq!(editor.history_index, 1);
        assert_eq!(editor.lines[0].annotation, Some("note".to_string()));
    }

    #[test]
    fn test_undo_redo_multiple() {
        let test_file = "test_undo_multi.txt";
        std::fs::write(test_file, "line1\nline2\nline3").unwrap();
        let mut editor = Editor::new(test_file.to_string()).unwrap();
        std::fs::remove_file(test_file).unwrap();

        // Action 1: Add annotation to line 0
        editor.lines[0].annotation = Some("note0".to_string());
        editor.perform_action(crate::models::Action::EditAnnotation {
            line_index: 0,
            old_text: None,
            new_text: Some("note0".to_string()),
        });

        // Action 2: Add annotation to line 1
        editor.lines[1].annotation = Some("note1".to_string());
        editor.perform_action(crate::models::Action::EditAnnotation {
            line_index: 1,
            old_text: None,
            new_text: Some("note1".to_string()),
        });

        // Action 3: Add annotation to line 2
        editor.lines[2].annotation = Some("note2".to_string());
        editor.perform_action(crate::models::Action::EditAnnotation {
            line_index: 2,
            old_text: None,
            new_text: Some("note2".to_string()),
        });

        assert_eq!(editor.history.len(), 3);
        assert_eq!(editor.history_index, 3);

        // Undo all three
        editor.undo(); // Undo note2
        assert_eq!(editor.lines[2].annotation, None);

        editor.undo(); // Undo note1
        assert_eq!(editor.lines[1].annotation, None);

        editor.undo(); // Undo note0
        assert_eq!(editor.lines[0].annotation, None);

        assert_eq!(editor.history_index, 0);

        // Redo all three
        editor.redo();
        assert_eq!(editor.lines[0].annotation, Some("note0".to_string()));

        editor.redo();
        assert_eq!(editor.lines[1].annotation, Some("note1".to_string()));

        editor.redo();
        assert_eq!(editor.lines[2].annotation, Some("note2".to_string()));

        assert_eq!(editor.history_index, 3);
    }

    #[test]
    fn test_undo_then_new_action_truncates_history() {
        let test_file = "test_undo_truncate.txt";
        std::fs::write(test_file, "line1").unwrap();
        let mut editor = Editor::new(test_file.to_string()).unwrap();
        std::fs::remove_file(test_file).unwrap();

        // Action 1
        editor.lines[0].annotation = Some("note1".to_string());
        editor.perform_action(crate::models::Action::EditAnnotation {
            line_index: 0,
            old_text: None,
            new_text: Some("note1".to_string()),
        });

        // Action 2
        editor.lines[0].annotation = Some("note2".to_string());
        editor.perform_action(crate::models::Action::EditAnnotation {
            line_index: 0,
            old_text: Some("note1".to_string()),
            new_text: Some("note2".to_string()),
        });

        assert_eq!(editor.history.len(), 2);

        // Undo once
        editor.undo();
        assert_eq!(editor.lines[0].annotation, Some("note1".to_string()));
        assert_eq!(editor.history_index, 1);

        // New action should truncate history
        editor.lines[0].annotation = Some("note3".to_string());
        editor.perform_action(crate::models::Action::EditAnnotation {
            line_index: 0,
            old_text: Some("note1".to_string()),
            new_text: Some("note3".to_string()),
        });

        // History should be truncated: only action1 and action3 remain
        assert_eq!(editor.history.len(), 2);
        assert_eq!(editor.history_index, 2);

        // Verify we can't redo to note2 anymore
        editor.undo();
        assert_eq!(editor.lines[0].annotation, Some("note1".to_string()));

        editor.redo();
        assert_eq!(editor.lines[0].annotation, Some("note3".to_string())); // Not note2!
    }

    #[test]
    fn test_is_modified_hash_based() {
        let test_file = "test_modified_hash.txt";
        std::fs::write(test_file, "line1").unwrap();
        let mut editor = Editor::new(test_file.to_string()).unwrap();

        // Initially not modified
        assert!(!editor.is_modified());

        // Add annotation - now modified
        editor.lines[0].annotation = Some("note".to_string());
        editor.perform_action(crate::models::Action::EditAnnotation {
            line_index: 0,
            old_text: None,
            new_text: Some("note".to_string()),
        });
        assert!(editor.is_modified());

        // Save clears modified state
        editor.save().unwrap();
        assert!(!editor.is_modified());

        // Undo after save - now modified again (different from saved state)
        editor.undo();
        assert!(editor.is_modified());

        // Cleanup
        std::fs::remove_file(test_file).unwrap();
    }

    #[test]
    fn test_is_modified_undo_to_original() {
        let test_file = "test_modified_undo.txt";
        std::fs::write(test_file, "line1").unwrap();
        let mut editor = Editor::new(test_file.to_string()).unwrap();

        // Initially not modified
        assert!(!editor.is_modified());

        // Add annotation - now modified
        editor.lines[0].annotation = Some("note".to_string());
        editor.perform_action(crate::models::Action::EditAnnotation {
            line_index: 0,
            old_text: None,
            new_text: Some("note".to_string()),
        });
        assert!(editor.is_modified());

        // Undo back to original - should NOT be modified anymore
        editor.undo();
        assert!(!editor.is_modified(), "Undoing to original state should not be modified");

        // Cleanup
        std::fs::remove_file(test_file).unwrap();
    }

    #[test]
    fn test_is_modified_add_then_delete_annotation() {
        let test_file = "test_modified_add_delete.txt";
        std::fs::write(test_file, "line1").unwrap();
        let mut editor = Editor::new(test_file.to_string()).unwrap();

        // Initially not modified
        assert!(!editor.is_modified());

        // Add annotation
        editor.lines[0].annotation = Some("note".to_string());
        editor.perform_action(crate::models::Action::EditAnnotation {
            line_index: 0,
            old_text: None,
            new_text: Some("note".to_string()),
        });
        assert!(editor.is_modified());

        // Delete annotation (back to original state)
        editor.lines[0].annotation = None;
        editor.perform_action(crate::models::Action::EditAnnotation {
            line_index: 0,
            old_text: Some("note".to_string()),
            new_text: None,
        });
        // Should NOT be modified - back to original state
        assert!(!editor.is_modified(), "Deleting annotation to match original should not be modified");

        // Cleanup
        std::fs::remove_file(test_file).unwrap();
    }

    #[test]
    fn test_state_transitions() {
        use crate::models::{ViewMode, EditorState};

        let test_file = "test_modes.txt";
        std::fs::write(test_file, "line1").unwrap();
        let mut editor = Editor::new(test_file.to_string()).unwrap();
        std::fs::remove_file(test_file).unwrap();

        // Starts in Normal view mode and Idle editor state
        assert!(matches!(editor.view_mode, ViewMode::Normal));
        assert!(matches!(editor.editor_state, EditorState::Idle));

        // Can transition editor_state to Searching
        editor.editor_state = EditorState::Searching {
            query: String::new(),
            cursor_pos: 0,
        };
        assert!(matches!(editor.editor_state, EditorState::Searching { .. }));

        // Can transition back to Idle
        editor.editor_state = EditorState::Idle;
        assert!(matches!(editor.editor_state, EditorState::Idle));

        // Can transition to Annotating
        editor.editor_state = EditorState::Annotating {
            buffer: "test".to_string(),
            cursor_pos: 4,
        };
        assert!(matches!(editor.editor_state, EditorState::Annotating { .. }));

        // Can transition to ShowingHelp
        editor.editor_state = EditorState::ShowingHelp;
        assert!(matches!(editor.editor_state, EditorState::ShowingHelp));

        // Can transition to QuitPrompt
        editor.editor_state = EditorState::QuitPrompt;
        assert!(matches!(editor.editor_state, EditorState::QuitPrompt));

        // ViewMode is independent - changing editor_state doesn't affect view_mode
        assert!(matches!(editor.view_mode, ViewMode::Normal));
    }

    #[test]
    fn test_cursor_bounds() {
        let test_file = "test_cursor.txt";
        std::fs::write(test_file, "line1\nline2\nline3").unwrap();
        let mut editor = Editor::new(test_file.to_string()).unwrap();
        std::fs::remove_file(test_file).unwrap();

        assert_eq!(editor.lines.len(), 3);
        assert_eq!(editor.cursor_line, 0);

        // Move cursor within bounds
        editor.cursor_line = 1;
        assert_eq!(editor.cursor_line, 1);

        editor.cursor_line = 2;
        assert_eq!(editor.cursor_line, 2);

        // Cursor at max valid position
        let max_line = editor.lines.len() - 1;
        editor.cursor_line = max_line;
        assert_eq!(editor.cursor_line, max_line);
    }
}
