#[derive(Clone)]
pub struct Line {
    pub content: String,
    pub annotation: Option<String>,
}

pub enum Mode {
    Normal,
    Annotating { buffer: String, cursor_pos: usize },
    Search { query: String, cursor_pos: usize },
    QuitPrompt,
    Help,
}

#[derive(Clone, Debug)]
pub enum Action {
    EditAnnotation {
        line_index: usize,
        old_text: Option<String>,
        new_text: Option<String>,
    },
}
