use std::{
    fs, io,
    path::{Path, PathBuf},
};

use crossterm::event::read;
use ratatui::{
    DefaultTerminal,
    layout::{Constraint, Direction, Layout},
    style::Style,
    widgets::{Block, Borders},
};
use tui_textarea::{Input, Key, TextArea};

use crate::{
    editor::Editor,
    homepage::{HomePage, InputResult},
    vim::{Mode, Search, Transition, Vim},
};

pub struct Vault<'a> {
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

    pub fn run(&mut self) -> io::Result<()> {
        let mut vim = Vim::new(Mode::Normal);
        if self.home.is_open() {
            self.home
                .textarea
                .set_block(Block::default().borders(Borders::ALL));
            self.home.update_homepage_files(&self.file_paths.clone());
        }
        loop {
            self.terminal.draw(|frame| {
                if self.home.is_open() {
                    frame.render_widget(&self.home.textarea, frame.area());
                } else {
                    frame.render_widget(&self.editor.textarea, frame.area());
                }
            })?;

            if self.home.is_open() {
                match self.home.input(&self.file_paths)? {
                    InputResult::Continue => continue,
                    InputResult::File(filename) => {
                        self.home.close();
                        self.open_file(filename)?;
                    }
                    InputResult::Quit => break,
                }
            } else {
                // TODO: switch back to event::read but the long line was messing up formating
                vim = match vim.exec(read()?.into(), &mut self.editor.textarea, &self.file_paths) {
                    Transition::Mode(mode) if vim.mode != mode => Vim::new(mode),
                    Transition::Nop | Transition::Mode(_) | Transition::InputResult(_) => vim,
                    Transition::Pending(input) => vim.with_pending(input),
                    Transition::Command => self.render_command_area()?,
                    Transition::Search(search) => match search {
                        Search::Open => {
                            let previous_search = {
                                if self.editor.textarea.search_pattern().is_some() {
                                    self.editor
                                        .textarea
                                        .search_pattern()
                                        .unwrap()
                                        .as_str()
                                        .to_string()
                                } else {
                                    "".to_string()
                                }
                            };
                            self.render_search_area(previous_search)?;
                            vim
                        }
                        Search::Forward => {
                            self.editor.textarea.search_forward(false);
                            vim
                        }
                        Search::Backward => {
                            self.editor.textarea.search_back(false);
                            vim
                        }
                    },
                }
            }
        }

        Ok(())
    }

    fn open_file(&mut self, path: PathBuf) -> io::Result<()> {
        self.editor.open(path)
    }

    fn render_command_area(&mut self) -> io::Result<Vim> {
        let mut command_area = TextArea::default();
        command_area.set_cursor_line_style(Style::default());

        loop {
            self.terminal
                .draw(|frame| {
                    command_area.set_block(Block::bordered().title("Command"));

                    let layout = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints(
                            [
                                // Layout for command area
                                // As tall as the cursor
                                Constraint::Length(3),
                                // The area for the editor
                                // Takes up the rest of the area
                                Constraint::Min(1),
                            ]
                            .as_ref(),
                        );

                    let chunks = layout.split(frame.area());

                    frame.render_widget(&command_area, chunks[0]);
                    frame.render_widget(&self.editor.textarea, chunks[1]);
                })
                .unwrap();

            match read()?.into() {
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
                            }
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
        Ok(Vim::new(Mode::Normal))
    }

    fn render_search_area(&mut self, previous_search: String) -> io::Result<Vim> {
        let mut search_area = TextArea::default();
        search_area.set_cursor_line_style(Style::default());

        loop {
            self.terminal
                .draw(|frame| {
                    search_area.set_block(Block::bordered().title("Search"));

                    let layout = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints(
                            [
                                // Layout for search area
                                // As tall as the cursor
                                Constraint::Length(3),
                                // The area for the editor
                                // Takes up the rest of the area
                                Constraint::Min(1),
                            ]
                            .as_ref(),
                        );

                    let chunks = layout.split(frame.area());

                    frame.render_widget(&search_area, chunks[0]);
                    frame.render_widget(&self.editor.textarea, chunks[1]);
                })
                .unwrap();

            match read()?.into() {
                Input {
                    key: Key::Enter, ..
                } => {
                    self.editor.textarea.search_forward(true);
                    break;
                }
                Input { key: Key::Esc, .. } => {
                    self.editor
                        .textarea
                        .set_search_pattern(previous_search)
                        .unwrap();
                    break;
                }
                input => {
                    search_area.input(input);
                    let lines = search_area.lines();
                    self.editor
                        .textarea
                        .set_search_pattern(lines[0].clone())
                        .unwrap();
                    self.editor.textarea.search_forward(true);
                }
            }
        }
        search_area.set_search_pattern("").unwrap();
        Ok(Vim::new(Mode::Normal))
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
    let paths: Vec<String> = 'block: {
        let paths: Vec<String> = args.skip(1).map(|arg| arg.into_string().unwrap()).collect();
        // If no dir provided use current dir
        if paths.len() == 0 {
            break 'block vec![".".to_string()];
        }
        paths
    };
    let root_directory = PathBuf::from(&paths[0]);

    let mut all_files: Vec<PathBuf> = Vec::new();
    populate_filenames(&root_directory, &mut all_files)?;

    Ok(all_files)
}
