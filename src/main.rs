use std::{
    fs::{self, File},
    io::{self, BufRead, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
};

use crossterm::event::{self};
use ratatui::{
    DefaultTerminal,
    layout::{Constraint, Direction, Flex, Layout, Rect},
    style::Style,
    widgets::{Block, Borders, Clear},
};
use tui_textarea::{Input, Key, TextArea};

use crate::vim::{Mode, Transition, Vim};

mod vim;

fn main() -> io::Result<()> {
    let mut vault = Vault::new();
    let result = vault.run();
    ratatui::restore();
    result
}

struct Vault<'a> {
    terminal: DefaultTerminal,
    editor: Editor<'a>,
    home: HomePage<'a>,
    file_paths: Vec<PathBuf>,
}

impl Vault<'_> {
    pub fn new() -> Self {
        let file_paths = get_all_filenames().unwrap();
        Self {
            terminal: ratatui::init(),
            editor: Editor::default(),
            home: HomePage::new(&file_paths),
            file_paths,
        }
    }

    fn run(&mut self) -> io::Result<()> {
        let mut vim = Vim::new(Mode::Normal);
        if self.home.open {
            self.home
                .textarea
                .set_block(Block::default().borders(Borders::ALL));
            self.home.update_homepage_files(&self.file_paths.clone());
        }
        loop {
            self.terminal.draw(|frame| {
                if self.home.open {
                    frame.render_widget(&self.home.textarea, frame.area());
                } else {
                    frame.render_widget(&self.editor.textarea, frame.area());
                }
            })?;

            if self.home.open {
                match event::read()?.into() {
                    Input { key: Key::Esc, .. } => break,
                    Input {
                        key: Key::Enter, ..
                    } => {
                        let (row, _) = self.home.textarea.cursor();
                        self.home.close();

                        let selected_file = self.file_paths[row].clone();
                        self.open_file(selected_file)?;
                    }
                    input => {
                        self.home.textarea.input(input);
                    }
                }
            } else {
                vim = match vim.exec(event::read()?.into(), &mut self.editor.textarea) {
                    Transition::Mode(mode) if vim.mode != mode => Vim::new(mode),
                    Transition::Nop | Transition::Mode(_) => vim,
                    Transition::Pending(input) => vim.with_pending(input),
                    Transition::Command => {
                        let mut command_area = TextArea::default();
                        command_area.set_cursor_line_style(Style::default());

                        loop {
                            self.terminal
                                .draw(|frame| {
                                    command_area.set_block(Block::bordered().title("Command"));

                                    // Determine the desired width and height for your text area
                                    let command_area_width = 50;
                                    let command_area_height = 3;

                                    // Create a centered Rect for the text area
                                    let area = center(
                                        frame.area(),
                                        Constraint::Length(command_area_width),
                                        Constraint::Length(command_area_height),
                                    );

                                    frame.render_widget(&self.editor.textarea, frame.area());
                                    frame.render_widget(Clear, area);
                                    frame.render_widget(&command_area, area);
                                })
                                .unwrap();

                            match event::read()?.into() {
                                Input { key: Key::Esc, .. } => break,
                                Input {
                                    key: Key::Enter, ..
                                } => {
                                    let input = command_area.lines();
                                    if input.len() == 1 {
                                        let input = &input[0];
                                        match input.as_str() {
                                            "q" => {
                                                self.home.open();
                                                break;
                                            },
                                            "w" => {
                                                self.editor.save()?;
                                                break;
                                            }
                                            _ => break,
                                        }
                                    }
                                }
                                input => {
                                    command_area.input(input);
                                }
                            }
                        }
                        Vim::new(Mode::Normal)
                    }
                }
            }
        }

        Ok(())
    }

    fn open_file(&mut self, path: PathBuf) -> io::Result<()> {
        self.editor.open(path)
    }
}

struct Editor<'a> {
    textarea: TextArea<'a>,
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
    fn open(&mut self, path: PathBuf) -> io::Result<()> {
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

    fn save(&self) -> io::Result<()> {
        let mut file = BufWriter::new(File::create(&self.path)?);
        for line in self.textarea.lines() {
            file.write_all(line.as_bytes())?;
            file.write_all(b"\n")?;
        }

        Ok(())
    }
}

struct HomePage<'a> {
    textarea: TextArea<'a>,
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

    fn update_homepage_files(&mut self, filenames: &Vec<PathBuf>) {
        *self = Self::new(filenames);
        self.open();
    }

    fn open(&mut self) {
        self.open = true;
    }

    fn close(&mut self) {
        self.open = false;
    }
}

fn populate_filenames(current_path: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    if current_path.is_dir() {
        for entry_result in fs::read_dir(current_path)? {
            let entry = entry_result?;
            let entry_path = entry.path();

            if entry_path.is_dir() {
                populate_filenames(&entry_path, files)?;
            } else if entry_path.is_file() {
                files.push(entry_path);
            }
        }
    }
    Ok(())
}

fn get_all_filenames() -> io::Result<Vec<PathBuf>> {
    let args = std::env::args_os();
    let paths: Vec<String> = args.skip(1).map(|arg| arg.into_string().unwrap()).collect();
    let root_directory = PathBuf::from(&paths[0]);

    let mut all_files: Vec<PathBuf> = Vec::new();
    populate_filenames(&root_directory, &mut all_files)?;

    Ok(all_files)
}

fn center(area: Rect, horizontal: Constraint, vertical: Constraint) -> Rect {
    let [area] = Layout::horizontal([horizontal])
        .flex(Flex::Center)
        .areas(area);
    let [area] = Layout::vertical([vertical]).flex(Flex::Center).areas(area);
    area
}
