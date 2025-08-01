use std::{
    fs::File,
    io::{self, BufRead, BufReader, BufWriter, Write},
    path::PathBuf,
};

use ratatui::style::Style;
use tui_textarea::TextArea;

#[derive(Clone, Debug)]
pub struct Editor<'a> {
    pub textarea: TextArea<'a>,
    path: PathBuf,
}

impl Editor<'_> {
    pub fn new() -> Self {
        Self {
            textarea: TextArea::default(),
            path: PathBuf::default(),
        }
    }

    fn style(&mut self) {
        self.textarea.set_line_number_style(Style::default());
    }

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

        self.style();

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
