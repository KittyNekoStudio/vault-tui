use std::{
    fs::{self, File},
    io::{self},
    path::{Path, PathBuf},
};

use crossterm::event::read;
use ratatui::{
    DefaultTerminal,
    layout::{Constraint, Direction, Layout},
    style::Style,
    widgets::Block,
};
use tui_textarea::{Input, Key, TextArea};

use crate::{
    buffer::Buffer,
    homepage::InputResult,
    vim::{Mode, Search, Transition, Vim},
};

#[derive(Debug)]
pub struct Vault<'a> {
    terminal: DefaultTerminal,
    current_buf: usize,
    // TODO: Change to hashmap of key: PathBuf value: Buffer
    pub buffers: Vec<Buffer<'a>>,
    file_paths: Vec<PathBuf>,
}

impl Vault<'_> {
    pub fn new<'a>() -> Vault<'a> {
        let file_paths = get_all_filenames().unwrap();
        let buffers = vec![Buffer::new_homepage(&file_paths)];
        Vault {
            terminal: ratatui::init(),
            current_buf: 0,
            buffers,
            file_paths,
        }
    }

    pub fn run(&mut self) -> io::Result<()> {
        let mut vim = Vim::new(Mode::Normal);

        if let Buffer::HomePage(homepage) = &mut self.buffers[0] {
            homepage.update_homepage_files(&self.file_paths);
        }

        loop {
            self.terminal.draw(|frame| {
                frame.render_widget(self.buffers[self.current_buf].textarea(), frame.area());
            })?;

            match &mut self.buffers[self.current_buf] {
                Buffer::HomePage(homepage) => match homepage.input(&self.file_paths)? {
                    InputResult::Continue => continue,
                    InputResult::File(filename) => {
                        self.open_file(filename)?;
                    }
                    InputResult::Search(search) => match search {
                        Search::Open => {
                            let previous_search = {
                                if homepage.textarea.search_pattern().is_some() {
                                    homepage
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
                        }
                        Search::Forward => {
                            homepage.textarea.search_forward(false);
                        }
                        Search::Backward => {
                            homepage.textarea.search_back(false);
                        }
                    },
                    InputResult::Command => _ = self.render_command_area()?,
                    InputResult::Quit => break,
                },
                Buffer::Editor(editor) => {
                    // TODO: switch back to event::read but the long line was messing up formating
                    vim = match vim.exec(read()?.into(), &mut editor.textarea, &self.file_paths) {
                        Transition::Mode(mode) if vim.mode != mode => Vim::new(mode),
                        Transition::Nop | Transition::Mode(_) | Transition::InputResult(_) => vim,
                        Transition::Pending(input) => vim.with_pending(input),
                        Transition::Command => self.render_command_area()?,
                        Transition::Search(search) => match search {
                            Search::Open => {
                                let previous_search = {
                                    if editor.textarea.search_pattern().is_some() {
                                        editor
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
                                editor.textarea.search_forward(false);
                                vim
                            }
                            Search::Backward => {
                                editor.textarea.search_back(false);
                                vim
                            }
                        },
                    }
                }
            }
        }

        Ok(())
    }

    fn add_editor(&mut self) {
        self.buffers.push(Buffer::new_editor());
        self.current_buf += 1;
    }

    fn open_file(&mut self, path: PathBuf) -> io::Result<()> {
        self.add_editor();
        if let Buffer::Editor(editor) = &mut self.buffers[self.current_buf] {
            editor.open(path)
        } else {
            Err(io::Error::new(io::ErrorKind::Other, "Not an editor"))
        }
    }

    fn render_command_area(&mut self) -> io::Result<Vim> {
        let mut command_area = TextArea::default();
        command_area.set_cursor_line_style(Style::default());
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

        loop {
            self.terminal
                .draw(|frame| {
                    let chunks = layout.split(frame.area());

                    frame.render_widget(&command_area, chunks[0]);
                    frame.render_widget(self.buffers[self.current_buf].textarea(), chunks[1]);
                })
                .unwrap();

            match &mut self.buffers[self.current_buf] {
                Buffer::Editor(editor) => match read()?.into() {
                    Input { key: Key::Esc, .. } => break,
                    Input {
                        key: Key::Enter, ..
                    } => {
                        let input = command_area.lines();
                        if input.len() == 1 {
                            let input = &input[0];
                            match input.as_str() {
                                "q" => {
                                    self.current_buf = 0;
                                    break;
                                }
                                "w" => {
                                    editor.save()?;
                                    break;
                                }
                                _ => break,
                            }
                        }
                    }
                    input => {
                        command_area.input(input);
                    }
                },
                Buffer::HomePage(_) => match read()?.into() {
                    Input { key: Key::Esc, .. } => break,
                    Input {
                        key: Key::Enter, ..
                    } => {
                        let input = command_area.lines();
                        match input[0].as_str() {
                            "new note" => {
                                self.new_note()?;
                                break;
                            }
                            _ => (),
                        }
                    }
                    input => {
                        command_area.input(input);
                    }
                },
            }
        }

        Ok(Vim::new(Mode::Normal))
    }

    fn render_search_area(&mut self, previous_search: String) -> io::Result<Vim> {
        let current = &mut self.buffers[self.current_buf];
        let textarea = match current {
            Buffer::Editor(editor) => &mut editor.textarea,
            Buffer::HomePage(homepage) => &mut homepage.textarea,
        };

        let mut search_area = TextArea::default();
        search_area.set_cursor_line_style(Style::default());

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

        loop {
            self.terminal
                .draw(|frame| {
                    let chunks = layout.split(frame.area());

                    frame.render_widget(&search_area, chunks[0]);
                    frame.render_widget(&textarea.clone(), chunks[1]);
                })
                .unwrap();

            match read()?.into() {
                Input {
                    key: Key::Enter, ..
                } => {
                    textarea.search_forward(true);
                    break;
                }
                Input { key: Key::Esc, .. } => {
                    textarea.set_search_pattern(previous_search).unwrap();
                    break;
                }
                input => {
                    search_area.input(input);
                    let lines = search_area.lines();
                    textarea.set_search_pattern(lines[0].clone()).unwrap();
                    textarea.search_forward(true);
                }
            }
        }
        search_area.set_search_pattern("").unwrap();
        Ok(Vim::new(Mode::Normal))
    }

    fn new_note(&mut self) -> io::Result<()> {
        let mut note_name_input = TextArea::default();
        note_name_input.set_cursor_line_style(Style::default());
        note_name_input.set_block(Block::bordered().title("Note Name"));

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    // Layout for note name area
                    // As tall as the cursor
                    Constraint::Length(3),
                    // The area for the editor
                    // Takes up the rest of the area
                    Constraint::Min(1),
                ]
                .as_ref(),
            );

        loop {
            self.terminal
                .draw(|frame| {
                    let chunks = layout.split(frame.area());

                    frame.render_widget(&note_name_input, chunks[0]);
                    frame.render_widget(self.buffers[self.current_buf].textarea(), chunks[1]);
                })
                .unwrap();

            match read()?.into() {
                Input {
                    key: Key::Enter, ..
                } => {
                    let input = note_name_input.lines();
                    File::create(&input[0])?;
                    self.open_file(PathBuf::from(&input[0]))?;
                    break;
                }
                input => {
                    note_name_input.input(input);
                }
            }
        }

        Ok(())
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
