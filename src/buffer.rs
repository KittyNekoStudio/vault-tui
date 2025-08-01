use std::path::PathBuf;

use tui_textarea::TextArea;

use crate::{editor::Editor, homepage::HomePage};

#[derive(Clone, Debug)]
pub enum Buffer<'a> {
    Editor(Editor<'a>),
    HomePage(HomePage<'a>),
}

impl Buffer<'_> {
    pub fn new_homepage(file_paths: &Vec<PathBuf>) -> Self {
        Buffer::HomePage(HomePage::new(file_paths))
    }

    pub fn new_editor() -> Self {
        Buffer::Editor(Editor::default())
    }

    pub fn textarea(&mut self) -> &TextArea {
        match self {
            Buffer::Editor(editor) => &editor.textarea,
            Buffer::HomePage(homepage) => &homepage.textarea,
        }
    }
}
