use std::{
    fs::File,
    io::{self, BufRead, BufReader, BufWriter, Write},
    path::PathBuf,
};

use ratatui::style::Style;
use tui_textarea::TextArea;

#[derive(Debug, Clone)]
pub struct Editor<'a> {
    pub textareas: Vec<TextArea<'a>>,
    pub paths: Vec<PathBuf>,
    pub current: usize,
}

impl Editor<'_> {
    pub fn new() -> Self {
        let textarea = TextArea::new(vec![
            "Press ':' and type search to search for notes".to_string(),
        ]);
        let path = PathBuf::from("vault-tui-intro-buffer");
        Self {
            textareas: vec![textarea],
            paths: vec![path],
            current: 0,
        }
    }

    pub fn path(&self) -> &PathBuf {
        &self.paths[self.current]
    }

    pub fn textarea(&self) -> &TextArea {
        &self.textareas[self.current]
    }

    pub fn open(&mut self, path: PathBuf) -> io::Result<()> {
        if self.textareas.len() != 0 {
            self.current = self.textareas.len();
        }

        let file = File::open(&path)?;
        let reader = BufReader::new(file);

        let mut lines = Vec::new();

        for line in reader.lines() {
            let line = line?;
            lines.push(line);
        }

        self.textareas.push(TextArea::new(lines));
        self.paths.push(path);

        self.textareas[self.current].set_line_number_style(Style::default());

        Ok(())
    }

    pub fn save(&self) -> io::Result<()> {
        if self.paths[self.current] != PathBuf::from("vault-tui-intro-buffer") {
            let mut file = BufWriter::new(File::create(&self.path())?);
            for line in self.textareas[self.current].lines() {
                file.write_all(line.as_bytes())?;
                file.write_all(b"\n")?;
            }
        }

        Ok(())
    }
}
