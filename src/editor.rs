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
}

impl Editor {
    pub fn new(file_path: Option<String>) -> io::Result<Self> {
        let (lines, lang_comment) = if let Some(ref path) = file_path {
            let content = fs::read_to_string(path)?;
            let comment = file::detect_comment_style(path);
            let lines = file::parse_file(&content, &comment);
            (lines, comment)
        } else {
            (vec![Line { content: String::new(), annotation: None }], "//".to_string())
        };

        Ok(Editor {
            lines,
            cursor_line: 0,
            scroll_offset: 0,
            mode: Mode::Normal,
            file_path,
            modified: false,
            theme: Theme::Dark,
            lang_comment,
            search_matches: Vec::new(),
            current_match: None,
            annotation_scroll: 0,
        })
    }

    pub fn save(&mut self) -> io::Result<()> {
        if let Some(ref path) = self.file_path {
            file::save_file(path, &self.lines, &self.lang_comment)?;
            self.modified = false;
        }
        Ok(())
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
                        
                        if !event_handler::handle_normal_mode(
                            key,
                            &mut self.lines,
                            &mut self.cursor_line,
                            &mut self.mode,
                            &mut self.theme,
                            &mut self.modified,
                            &mut self.annotation_scroll,
                            &mut self.scroll_offset,
                        )? {
                            break;
                        }
                    }
                    Mode::Annotating { .. } => {
                        let Mode::Annotating { mut buffer, mut cursor_pos } = std::mem::replace(&mut self.mode, Mode::Normal) else {
                            unreachable!()
                        };
                        event_handler::handle_annotation_mode(
                            key,
                            &mut buffer,
                            &mut cursor_pos,
                            &mut self.lines,
                            self.cursor_line,
                            &mut self.mode,
                            &mut self.modified,
                            &mut self.annotation_scroll,
                        )?;
                        if matches!(self.mode, Mode::Normal) && key.code != KeyCode::Enter && key.code != KeyCode::Esc {
                            self.mode = Mode::Annotating { buffer, cursor_pos };
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
    fn test_editor_new_without_file() {
        let editor = Editor::new(None).unwrap();
        assert_eq!(editor.lines.len(), 1);
        assert_eq!(editor.cursor_line, 0);
        assert_eq!(editor.lang_comment, "//");
        assert!(!editor.modified);
    }

    #[test]
    fn test_search_functionality() {
        let mut editor = Editor::new(None).unwrap();
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
        let mut editor = Editor::new(None).unwrap();
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
}
