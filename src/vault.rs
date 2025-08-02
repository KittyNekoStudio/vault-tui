use std::{
    collections::HashMap,
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
    command::Command,
    homepage::InputResult,
    vim::{Mode, Search, Transition, Vim},
};

#[derive(Debug)]
pub struct Vault<'a> {
    terminal: DefaultTerminal,
    current_buf: PathBuf,
    previous_buf: PathBuf,
    buffers: HashMap<PathBuf, Buffer<'a>>,
    file_paths: Vec<PathBuf>,
    run: bool,
}

impl Vault<'_> {
    pub fn new<'a>() -> Vault<'a> {
        let file_paths = get_all_filenames(false).unwrap();
        let mut hashmap = HashMap::new();
        hashmap.insert(
            PathBuf::from("vault-homepage"),
            Buffer::new_homepage(&file_paths),
        );

        Vault {
            terminal: ratatui::init(),
            current_buf: PathBuf::from("vault-homepage"),
            previous_buf: PathBuf::from("vault-homepage"),
            buffers: hashmap,
            file_paths,
            run: true,
        }
    }

    pub fn run(&mut self) -> io::Result<()> {
        let mut vim = Vim::new(Mode::Normal);

        // When provided with a file instead of a dir
        // Open the file then update self.file_paths and the homepage with pwd
        if self.file_paths.len() == 1 {
            self.open_file(self.file_paths[0].clone())?;
            self.file_paths = get_all_filenames(true).unwrap();
            if let Buffer::HomePage(homepage) = &mut self
                .buffers
                .get_mut(&PathBuf::from("vault-homepage"))
                .unwrap()
            {
                homepage.update_homepage_files(&self.file_paths);
            }
        }

        while self.run {
            self.terminal.draw(|frame| {
                frame.render_widget(
                    self.buffers.get_mut(&self.current_buf).unwrap().textarea(),
                    frame.area(),
                );
            })?;

            match self.buffers.get_mut(&self.current_buf).unwrap() {
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
                    InputResult::CommandExec(command) => self.exec_command(command)?,
                },
                Buffer::Editor(editor) => {
                    // TODO: switch back to event::read but the long line was messing up formating
                    vim = match vim.exec(read()?.into(), &mut editor.textarea, &self.file_paths) {
                        Transition::Mode(mode) if vim.mode != mode => Vim::new(mode),
                        Transition::Nop | Transition::Mode(_) | Transition::InputResult(_) => vim,
                        Transition::Pending(input) => vim.with_pending(input),
                        Transition::CommandMode => self.render_command_area()?,
                        Transition::CommandExec(command) => {
                            self.exec_command(command)?;
                            vim
                        }
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

    fn open_file(&mut self, path: PathBuf) -> io::Result<()> {
        self.previous_buf = self.current_buf.clone();
        if self.buffers.contains_key(&path) {
            self.current_buf = path;
            return Ok(());
        }

        self.buffers.insert(path.clone(), Buffer::new_editor());
        self.current_buf = path.clone();

        if let Buffer::Editor(editor) = self.buffers.get_mut(&self.current_buf).unwrap() {
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
                    frame.render_widget(
                        self.buffers.get_mut(&self.current_buf).unwrap().textarea(),
                        chunks[1],
                    );
                })
                .unwrap();

            match read()?.into() {
                Input { key: Key::Esc, .. } => break,
                Input {
                    key: Key::Enter, ..
                } => {
                    let input = command_area.lines();
                    self.exec_command(Command::str_to_command(&input[0]))?;
                    break;
                }
                input => {
                    command_area.input(input);
                }
            }
        }

        Ok(Vim::new(Mode::Normal))
    }

    fn render_search_area(&mut self, previous_search: String) -> io::Result<Vim> {
        let current = self.buffers.get_mut(&self.current_buf).unwrap();
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
                    frame.render_widget(
                        self.buffers.get_mut(&self.current_buf).unwrap().textarea(),
                        chunks[1],
                    );
                })
                .unwrap();

            match read()?.into() {
                Input {
                    key: Key::Enter, ..
                } => {
                    let input = note_name_input.lines();
                    let pathbuf = PathBuf::from(&input[0]);

                    self.file_paths.push(pathbuf.clone());

                    File::create(&pathbuf)?;
                    self.open_file(pathbuf)?;

                    if let Buffer::HomePage(homepage) = self
                        .buffers
                        .get_mut(&PathBuf::from("vault-homepage"))
                        .unwrap()
                    {
                        homepage.update_homepage_files(&self.file_paths);
                    }
                    break;
                }
                Input { key: Key::Esc, .. } => break,
                input => {
                    note_name_input.input(input);
                }
            }
        }

        Ok(())
    }

    fn exec_command(&mut self, command: Command) -> io::Result<()> {
        match command {
            Command::Quit => {
                self.run = false;
            }
            Command::Save => {
                if let Buffer::Editor(editor) = &self.buffers[&self.current_buf] {
                    editor.save()?;
                }
            }
            Command::SaveQuit => {
                if let Buffer::Editor(editor) = &self.buffers[&self.current_buf] {
                    editor.save()?;
                }
                self.run = false;
            }
            Command::Home => {
                let current_buf = self.current_buf.clone();
                self.current_buf = PathBuf::from("vault-homepage");
                self.previous_buf = current_buf;
            }
            Command::NewNote => {
                self.new_note()?;
            }
            Command::FollowLink => {
                if let Buffer::Editor(editor) = &self.buffers[&self.current_buf] {
                    let (row, col) = editor.textarea.cursor();
                    let current_line = &editor.textarea.lines()[row];

                    let col = if col + 1 > current_line.len() {
                        col
                    } else {
                        col + 1
                    };

                    let line_split = current_line.split_at(col);

                    if line_split.0.contains("[[") && !line_split.0.contains("]]") {
                        let bracket_start_idx = current_line.find("[[").unwrap() + 2;
                        let bracket_end_idx = current_line.find("]]").unwrap();
                        let inside_filename = &current_line[bracket_start_idx..bracket_end_idx];

                        let filename = inside_filename.split("|").collect::<Vec<&str>>()[0];
                        self.open_file(PathBuf::from(filename.to_string() + ".md"))?;
                    }
                }
            }
            Command::PreviousBuf => {
                let current_buf = self.current_buf.clone();
                self.current_buf = self.previous_buf.clone();
                self.previous_buf = current_buf;
            }
            Command::None => (),
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
                let path = 'block: {
                    let path = entry_path.strip_prefix(".");
                    if path.is_err() {
                        break 'block entry_path.strip_prefix("/").unwrap();
                    }
                    path.unwrap()
                };
                files.push(path.to_path_buf());
            }
        }
    } else if current_path.is_file() {
        files.push(current_path.to_path_buf());
    }
    Ok(())
}

fn get_all_filenames(use_current_dir: bool) -> io::Result<Vec<PathBuf>> {
    let args = std::env::args_os();
    let paths: Vec<String> = 'block: {
        let paths: Vec<String> = args.skip(1).map(|arg| arg.into_string().unwrap()).collect();
        // If no dir provided use current dir
        if paths.len() == 0 || use_current_dir {
            break 'block vec![".".to_string()];
        }
        paths
    };
    let root_directory = PathBuf::from(&paths[0]);

    let mut all_files: Vec<PathBuf> = Vec::new();
    populate_filenames(&root_directory, &mut all_files)?;

    Ok(all_files)
}
