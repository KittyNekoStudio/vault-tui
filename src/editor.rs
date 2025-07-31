use std::{
    fs::File,
    io::{self, BufRead, BufReader, BufWriter, Write},
    path::PathBuf,
};

use ratatui::widgets::{Block, Borders};
use tui_textarea::TextArea;

pub struct Editor<'a> {
    pub textarea: TextArea<'a>,
    path: PathBuf,
}

impl Default for Editor<'_> {
    fn default() -> Self {
        let mut textarea = TextArea::default();
        textarea.set_block(Block::default().borders(Borders::ALL));
        Self {
            textarea,
            path: PathBuf::default(),
        }
    }
}

impl Editor<'_> {
    pub fn open(&mut self, path: PathBuf) -> io::Result<()> {
        let file = File::open(&path)?;
        let reader = BufReader::new(file);

        let mut lines = Vec::new();

        for line in reader.lines() {
            let line = line?;
            lines.push(line);
        }

        self.textarea = TextArea::new(lines);
        self.path = path;

        Ok(())
    }

    pub fn save(&self) -> io::Result<()> {
        let mut file = BufWriter::new(File::create(&self.path)?);
        for line in self.textarea.lines() {
            file.write_all(line.as_bytes())?;
            file.write_all(b"\n")?;
        }

        Ok(())
    }
}
