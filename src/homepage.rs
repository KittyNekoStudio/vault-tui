use std::path::PathBuf;

use ratatui::widgets::{Block, Borders};
use tui_textarea::TextArea;

pub struct HomePage<'a> {
    pub textarea: TextArea<'a>,
    open: bool,
}

impl HomePage<'_> {
    pub fn new(filenames: &Vec<PathBuf>) -> Self {
        let filenames = filenames
            .iter()
            .map(|name| name.clone().into_os_string().into_string().unwrap())
            .collect();
        let mut textarea = TextArea::new(filenames);
        textarea.set_block(Block::default().borders(Borders::ALL));
        Self {
            textarea,
            open: true,
        }
    }

    pub fn update_homepage_files(&mut self, filenames: &Vec<PathBuf>) {
        *self = Self::new(filenames);
        self.open();
    }

    pub fn open(&mut self) {
        self.open = true;
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn is_open(&self) -> bool {
        self.open
    }
}
