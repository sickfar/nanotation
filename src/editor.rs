use crate::event_handler;
use crate::file;
use crate::models::{Line, Mode};
use crate::theme::Theme;
use crate::ui;
use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event, KeyCode},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io;
use std::fs;

pub struct Editor {
    pub lines: Vec<Line>,
    pub cursor_line: usize,
    pub scroll_offset: usize,
    pub mode: Mode,
    pub file_path: Option<String>,
    pub modified: bool,
    pub theme: Theme,
    pub lang_comment: String,
    pub search_matches: Vec<usize>,
    pub current_match: Option<usize>,
    pub annotation_scroll: usize,
    pub history: Vec<crate::models::Action>,
    pub history_index: usize,
    pub highlighter: crate::highlighting::SyntaxHighlighter,
}

impl Editor {
    pub fn new(file_path: String) -> io::Result<Self> {
        let content = fs::read_to_string(&file_path)?;
        let lang_comment = file::detect_comment_style(&file_path);
        let lines = file::parse_file(&content, &lang_comment);
        let theme = Theme::Dark;
        let highlighter = crate::highlighting::SyntaxHighlighter::new(matches!(theme, Theme::Dark));

        Ok(Editor {
            lines,
            cursor_line: 0,
            scroll_offset: 0,
            mode: Mode::Normal,
            file_path: Some(file_path),
            modified: false,
            theme,
            lang_comment,
            search_matches: Vec::new(),
            current_match: None,
            annotation_scroll: 0,
            history: Vec::new(),
            history_index: 0,
            highlighter,
        })
    }

    pub fn save(&mut self) -> io::Result<()> {
        if let Some(ref path) = self.file_path {
            file::save_file(path, &self.lines, &self.lang_comment)?;
            self.modified = false;
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
        self.modified = true;
    }

    pub fn undo(&mut self) {
        if self.history_index > 0 {
            self.history_index -= 1;
            match &self.history[self.history_index] {
                crate::models::Action::EditAnnotation { line_index, old_text, .. } => {
                    self.lines[*line_index].annotation = old_text.clone();
                }
            }
            self.modified = true;
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
            self.modified = true;
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
            ui::render(
                &self.lines,
                self.cursor_line,
                self.scroll_offset,
                &self.mode,
                &self.file_path,
                self.modified,
                self.theme,
                &self.search_matches,
                self.current_match,
                self.annotation_scroll,
                &self.highlighter,
            )?;

            if let Event::Key(key) = event::read()? {
                match &self.mode {
                    Mode::Normal => {
                        // Handle save separately
                        if key.code == KeyCode::Char('o') && key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                            self.save()?;
                            continue;
                        }
                        
                        // Handle search mode entry
                        if key.code == KeyCode::Char('w') && key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                            self.mode = Mode::Search { query: String::new(), cursor_pos: 0 };
                            continue;
                        }
                        
                        let current_theme_is_dark = matches!(self.theme, crate::theme::Theme::Dark);
                        
                        match event_handler::handle_normal_mode(
                            key,
                            &mut self.lines,
                            &mut self.cursor_line,
                            &mut self.mode,
                            &mut self.theme,
                            &mut self.modified,
                            &mut self.annotation_scroll,
                            &mut self.scroll_offset,
                        )? {
                            event_handler::NormalModeResult::Exit => break,
                            event_handler::NormalModeResult::Action(action) => {
                                // Apply and push history
                                match &action {
                                    crate::models::Action::EditAnnotation { line_index, new_text, .. } => {
                                        self.lines[*line_index].annotation = new_text.clone();
                                    }
                                }
                                self.perform_action(action);
                            },
                            event_handler::NormalModeResult::Undo => self.undo(),
                            event_handler::NormalModeResult::Redo => self.redo(),
                            event_handler::NormalModeResult::Continue => {
                                // Check if theme changed
                                let new_theme_is_dark = matches!(self.theme, crate::theme::Theme::Dark);
                                if current_theme_is_dark != new_theme_is_dark {
                                    self.highlighter = crate::highlighting::SyntaxHighlighter::new(new_theme_is_dark);
                                }
                            },
                        }
                    }
                    Mode::Annotating { .. } => {
                        let Mode::Annotating { mut buffer, mut cursor_pos } = std::mem::replace(&mut self.mode, Mode::Normal) else {
                            unreachable!()
                        };
                        match event_handler::handle_annotation_mode(
                            key,
                            &mut buffer,
                            &mut cursor_pos,
                            &mut self.lines,
                            self.cursor_line,
                            &mut self.mode,
                            &mut self.modified,
                            &mut self.annotation_scroll,
                        )? {
                            Some(action) => {
                                // Apply and push history
                                match &action {
                                    crate::models::Action::EditAnnotation { line_index, new_text, .. } => {
                                        self.lines[*line_index].annotation = new_text.clone();
                                    }
                                }
                                self.perform_action(action);
                            },
                            None => {
                                if matches!(self.mode, Mode::Normal) && key.code != KeyCode::Enter && key.code != KeyCode::Esc {
                                    self.mode = Mode::Annotating { buffer, cursor_pos };
                                }
                            }
                        }
                    }
                    Mode::Search { .. } => {
                        let Mode::Search { mut query, mut cursor_pos } = std::mem::replace(&mut self.mode, Mode::Normal) else {
                            unreachable!()
                        };
                        event_handler::handle_search_mode(
                            key,
                            &mut query,
                            &mut cursor_pos,
                            &mut self.mode,
                            &mut self.search_matches,
                            &mut self.current_match,
                            &self.lines,
                            &mut self.cursor_line,
                            &mut self.scroll_offset,
                        )?;
                        if matches!(self.mode, Mode::Normal) && key.code != KeyCode::Esc {
                            self.mode = Mode::Search { query, cursor_pos };
                        }
                    }
                    Mode::QuitPrompt => {
                        match event_handler::handle_quit_prompt(key) {
                            event_handler::QuitPromptResult::SaveAndExit => {
                                self.save()?;
                                break;
                            }
                            event_handler::QuitPromptResult::Exit => {
                                break;
                            }
                            event_handler::QuitPromptResult::Cancel => {
                                self.mode = Mode::Normal;
                            }
                            event_handler::QuitPromptResult::Continue => {}
                        }
                    }
                    Mode::Help => {
                        // Any key exits help
                        self.mode = Mode::Normal;
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
}
