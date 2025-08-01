use std::{
    io::{self, Error},
    path::PathBuf,
};

use crossterm::event;
use ratatui::widgets::{Block, Borders};
use tui_textarea::TextArea;

use crate::vim::{Mode, Search, Transition, Vim};

#[derive(Clone, Debug)]
pub struct HomePage<'a> {
    pub textarea: TextArea<'a>,
}

pub enum InputResult {
    Quit,
    Continue,
    File(PathBuf),
    Search(Search),
}

impl HomePage<'_> {
    pub fn new(file_paths: &Vec<PathBuf>) -> Self {
        let file_paths = file_paths
            .iter()
            .map(|name| name.clone().into_os_string().into_string().unwrap())
            .collect();
        let mut textarea = TextArea::new(file_paths);
        textarea.set_block(Block::default().borders(Borders::ALL));
        Self { textarea }
    }

    pub fn update_homepage_files(&mut self, file_paths: &Vec<PathBuf>) {
        *self = Self::new(file_paths);
    }

    pub fn input(&mut self, file_paths: &Vec<PathBuf>) -> io::Result<InputResult> {
        let mut vim = Vim::new(Mode::HomePage);

        if let Transition::InputResult(input_result) =
            vim.exec(event::read()?.into(), &mut self.textarea, file_paths)
        {
            return Ok(input_result);
        } else {
            return Err(Error::new(
                io::ErrorKind::Other,
                "failed to match input result",
            ));
        }
    }
}
