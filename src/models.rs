#[derive(Clone)]
pub struct Line {
    pub content: String,
    pub annotation: Option<String>,
}

pub enum Mode {
    Normal,
    Annotating { buffer: String, cursor_pos: usize },
    Search { query: String, cursor_pos: usize },
}
