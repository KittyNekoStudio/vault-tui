use std::{fs, io, path::{Path, PathBuf}};

use crossterm::event;
use ratatui::{layout::{Constraint, Flex, Layout, Rect}, style::Style, widgets::{Block, Borders, Clear}, DefaultTerminal};
use tui_textarea::{Input, Key, TextArea};

use crate::{editor::Editor, homepage::HomePage, vim::{Mode, Transition, Vim}};

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
