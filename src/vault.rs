use std::{
    fs::{self, File},
    io::{self},
    path::{Path, PathBuf},
};

use chrono::Local;
use crossterm::event::{Event, read};
use fuzzy_matcher::{FuzzyMatcher, skim::SkimMatcherV2};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect}, style::{Modifier, Style}, text::{Line, Span}, widgets::{Block, Clear, Paragraph}, DefaultTerminal
};
use tui_textarea::{Input, Key, TextArea};

use crate::{
    command::Command,
    editor::Editor,
    error::VaultError,
    vim::{Mode, Search, Transition, Vim},
};

#[derive(Debug)]
pub struct Vault<'a> {
    terminal: DefaultTerminal,
    tabs: Vec<Editor<'a>>,
    current_tab: usize,
    vim: Vim,
    file_paths: Vec<PathBuf>,
    run: bool,
}

impl Vault<'_> {
    pub fn new<'a>() -> Vault<'a> {
        let file_paths = get_all_filenames(false).unwrap();

        Vault {
            terminal: ratatui::init(),
            tabs: vec![Editor::new()],
            current_tab: 0,
            vim: Vim::new(Mode::Normal),
            file_paths,
            run: true,
        }
    }

    pub fn run(&mut self) {
        // When provided with a file instead of a dir
        // Open the file then update self.file_paths and the homepage with pwd
        if self.file_paths.len() == 1 {
            let result = self.open_file(self.file_paths[0].clone());
            self.handle_error(result);
            self.file_paths = get_all_filenames(true).unwrap();
        }

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)].as_ref());

        while self.run {
            let status_bar = {
                let mut status_bar = Vec::new();

                for i in 0..self.tabs.len() {
                    if i == self.current_tab {
                        status_bar.push(Span::styled(
                            format!("{}", i),
                            Style::default().add_modifier(Modifier::UNDERLINED),
                        ));
                    } else {
                        status_bar.push(Span::styled(format!("{}", i), Style::default()));
                    }
                    status_bar.push(Span::styled(" ", Style::default()));
                }

                Line::from(status_bar)
            };

            // TODO: handle this error
            self.terminal
                .draw(|frame| {
                    let chunks = layout.split(frame.area());

                    frame.render_widget(self.tabs[self.current_tab].textarea(), chunks[0]);
                    frame.render_widget(Paragraph::new(status_bar), chunks[1]);
                })
                .unwrap();

            let result = self.input();
            self.handle_error(result);
        }
    }

    fn read() -> Result<Event, VaultError> {
        let input = read();
        if input.is_err() {
            return Err(VaultError::Input);
        }
        Ok(input.unwrap())
    }

    fn handle_error<T>(&mut self, result: Result<T, VaultError>) {
        if result.is_err() {
            match result.err().unwrap() {
                VaultError::OpenFile(filepath) => {
                    let result = self.render_notification_area(filepath);
                    self.handle_error(result);
                }
                VaultError::Input => {
                    let result = self.render_notification_area("Failed to read input".to_string());
                    self.handle_error(result);
                }
            }
        }
    }

    fn input(&mut self) -> Result<(), VaultError> {
        let tab = &mut self.tabs[self.current_tab];
        self.vim = match self
            .vim
            .exec(Self::read()?.into(), &mut tab.textareas[tab.current])
        {
            Transition::Mode(mode) if self.vim.mode != mode => Vim::new(mode),
            Transition::Nop | Transition::Mode(_) => self.vim.clone(),
            Transition::Pending(input) => self.vim.with_pending(input),
            Transition::CommandMode => {
                let vim = self.render_command_area()?;
                vim
            }
            Transition::CommandExec(command) => {
                self.exec_command(command)?;
                self.vim.clone()
            }
            Transition::Search(search) => match search {
                Search::Open => {
                    let previous_search = {
                        if self.tabs[self.current_tab]
                            .textarea()
                            .search_pattern()
                            .is_some()
                        {
                            self.tabs[self.current_tab]
                                .textarea()
                                .search_pattern()
                                .unwrap()
                                .as_str()
                                .to_string()
                        } else {
                            "".to_string()
                        }
                    };
                    self.render_search_area(previous_search)?;
                    return Ok(());
                }
                Search::Forward => {
                    let tab = &mut self.tabs[self.current_tab];
                    tab.textareas[tab.current].search_forward(false);
                    return Ok(());
                }
                Search::Backward => {
                    let tab = &mut self.tabs[self.current_tab];
                    tab.textareas[tab.current].search_back(false);
                    return Ok(());
                }
            },
            Transition::AutoComplete => {
                let mut link = "".to_string();
                let (row, _) = self.tabs[self.current_tab].textarea().cursor();
                let lines = self.tabs[self.current_tab].textarea().lines();

                if lines[row].contains("[[") && !lines[row].contains("]]") {
                    let inner_link = self.render_autocomplete()?;
                    if inner_link == "" {
                        return Ok(());
                    }
                    // Remove the .md file extension
                    let inner_link = &inner_link[0..inner_link.len() - 3];
                    let inner_link = inner_link.to_string() + "]]";
                    link = inner_link;
                }
                let (row, _) = self.tabs[self.current_tab].textarea().cursor();
                let lines = self.tabs[self.current_tab].textarea().lines();
                let start = lines[row].to_string().find("[[").unwrap() + 2;
                let current = &mut self.tabs[self.current_tab];
                current.textareas[current.current]
                    .move_cursor(tui_textarea::CursorMove::Jump(row as u16, start as u16));
                current.textareas[current.current].delete_line_by_end();
                current.textareas[current.current].insert_str(link);
                return Ok(());
            }
        };
        Ok(())
    }

    fn open_file(&mut self, path: PathBuf) -> Result<(), VaultError> {
        for i in 0..self.tabs[self.current_tab].textareas.len() {
            let tab = &mut self.tabs[self.current_tab];
            if &tab.paths[i] == &path {
                tab.current = i;
                return Ok(());
            }
        }

        self.tabs[self.current_tab].open(path.clone())?;

        Ok(())
    }

    fn open_template(path: PathBuf) -> Result<Editor<'static>, VaultError> {
        let mut editor = Editor::new();
        editor.open(path)?;
        Ok(editor)
    }

    fn render_command_area(&mut self) -> Result<Vim, VaultError> {
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

                    frame.render_widget(self.tabs[self.current_tab].textarea(), chunks[1]);

                    frame.render_widget(&command_area, chunks[0]);
                })
                .unwrap();

            match Self::read()?.into() {
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

    fn render_autocomplete(&mut self) -> Result<String, VaultError> {
        let scores = {
            let mut scores: Vec<(String, i64)> = Vec::new();
            let editor = &self.tabs[self.current_tab];
            let (row, _) = editor.textarea().cursor();
            let lines = editor.textarea().lines();

            let matcher = SkimMatcherV2::default();
            let start = lines[row].to_string().find("[[").unwrap() + 2;
            for file in &self.file_paths {
                let to_match = &lines[row][start..];
                let matched = matcher.fuzzy_match(file.to_str().unwrap(), to_match);
                if matched.is_some() {
                    scores.push((file.to_str().unwrap().to_string(), matched.unwrap()));
                } else {
                    continue;
                }
            }
            scores.sort();
            scores.reverse();
            scores
        };

        let mut autocomplete_area = TextArea::default();
        autocomplete_area.set_cursor_line_style(Style::default());
        autocomplete_area.set_block(Block::bordered());

        for (name, _) in scores {
            autocomplete_area.insert_str(name);
            autocomplete_area.insert_newline();
        }

        autocomplete_area.move_cursor(tui_textarea::CursorMove::Jump(0, 0));

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(10)].as_ref());

        loop {
            self.terminal
                .draw(|frame| {
                    let chunks = layout.split(frame.area());

                    frame.render_widget(&autocomplete_area, chunks[1]);
                    frame.render_widget(self.tabs[self.current_tab].textarea(), chunks[0]);
                })
                .unwrap();

            match Self::read()?.into() {
                Input { key: Key::Esc, .. } => break,
                Input {
                    key: Key::Enter, ..
                }
                | Input {
                    key: Key::Char('y'),
                    ctrl: true,
                    ..
                } => {
                    let (row, _) = autocomplete_area.cursor();
                    let lines = autocomplete_area.lines();

                    return Ok(lines[row].clone());
                }
                input => {
                    autocomplete_area.input(input);
                }
            }
        }

        Ok("".to_string())
    }

    fn render_file_search(&mut self) -> Result<String, VaultError> {
        let mut note_search_area = TextArea::default();
        note_search_area.set_cursor_line_style(Style::default());
        note_search_area.set_block(Block::bordered().title("Note Search"));

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Length(3),
                    Constraint::Length(10),
                    Constraint::Min(1),
                ]
                .as_ref(),
            );

        let mut autocomplete_cursor = (0, 0);
        loop {
            let scores = {
                let mut scores: Vec<(String, i64)> = Vec::new();
                let lines = note_search_area.lines();

                let matcher = SkimMatcherV2::default();
                for file in &self.file_paths {
                    let to_match = &lines[0];
                    let matched = matcher.fuzzy_match(file.to_str().unwrap(), to_match);
                    if matched.is_some() {
                        scores.push((file.to_str().unwrap().to_string(), matched.unwrap()));
                    } else {
                        continue;
                    }
                }
                scores.sort();
                scores.reverse();
                scores
            };

            let mut autocomplete_area = TextArea::default();
            autocomplete_area.set_cursor_line_style(Style::default());
            autocomplete_area.set_block(Block::bordered());

            for (name, _) in scores {
                autocomplete_area.insert_str(name);
                autocomplete_area.insert_newline();
            }

            autocomplete_area.move_cursor(tui_textarea::CursorMove::Jump(
                autocomplete_cursor.0,
                autocomplete_cursor.1,
            ));

            self.terminal
                .draw(|frame| {
                    let chunks = layout.split(frame.area());

                    frame.render_widget(&note_search_area, chunks[0]);
                    frame.render_widget(&autocomplete_area, chunks[1]);
                    frame.render_widget(self.tabs[self.current_tab].textarea(), chunks[2]);
                })
                .unwrap();

            match Self::read()?.into() {
                Input { key: Key::Esc, .. } => break,
                Input {
                    key: Key::Enter, ..
                }
                | Input {
                    key: Key::Char('y'),
                    ctrl: true,
                    ..
                } => {
                    let (row, _) = autocomplete_area.cursor();
                    let lines = autocomplete_area.lines();

                    return Ok(lines[row].clone());
                }
                input => {
                    if input
                        == (Input {
                            key: Key::Char('n'),
                            ctrl: true,
                            alt: false,
                            shift: false,
                        })
                        || input
                            == (Input {
                                key: Key::Char('p'),
                                ctrl: true,
                                alt: false,
                                shift: false,
                            })
                    {
                        autocomplete_area.input(input);
                        let (row, col) = autocomplete_area.cursor();
                        autocomplete_cursor = (row as u16, col as u16);
                    } else {
                        note_search_area.input(input);
                        autocomplete_cursor = (0, 0);
                    }
                }
            }
        }

        Ok("".to_string())
    }

    fn render_search_area(&mut self, previous_search: String) -> Result<Vim, VaultError> {
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
                    let tab = &self.tabs[self.current_tab];

                    frame.render_widget(&tab.textareas[tab.current], chunks[1]);
                    frame.render_widget(&search_area, chunks[0]);
                })
                .unwrap();

            let textarea: &mut TextArea = {
                let tab = &mut self.tabs[self.current_tab];
                &mut tab.textareas[tab.current]
            };

            match Self::read()?.into() {
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

    fn render_notification_area(&mut self, notification: String) -> Result<(), VaultError> {
        let mut notification_area = TextArea::default();

        notification_area.set_cursor_line_style(Style::default());
        notification_area.set_block(Block::bordered().title("Notification"));
        notification_area.insert_str(notification);

        loop {
            self.terminal
                .draw(|frame| {
                    let w = 50;
                    let h = 5;
                    let x = frame.area().width - w;
                    let y = frame.area().y;
                    let rect = Rect::new(x, y, w, h);
                    let tab = &self.tabs[self.current_tab];

                    frame.render_widget(&tab.textareas[tab.current], frame.area());
                    frame.render_widget(Clear, rect);
                    frame.render_widget(&notification_area, rect);
                })
                .unwrap();

            match Self::read()?.into() {
                Input { key: Key::Esc, .. } => {
                    break;
                }
                _ => (),
            }
        }

        Ok(())
    }

    fn new_note(&mut self) -> Result<(), VaultError> {
        let mut note_name_area = TextArea::default();
        note_name_area.set_cursor_line_style(Style::default());
        note_name_area.set_block(Block::bordered().title("Note Name"));

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

                    let tab = &self.tabs[self.current_tab];
                    frame.render_widget(&tab.textareas[tab.current], chunks[1]);

                    frame.render_widget(&note_name_area, chunks[0]);
                })
                .unwrap();

            match Self::read()?.into() {
                Input {
                    key: Key::Enter, ..
                } => {
                    // Make it so the user does not need to provide the file extension
                    note_name_area.insert_str(".md");

                    let filename = get_formated_date("{{date:YMMDDHHmm-}}".to_string())
                        + &note_name_area.lines()[0];

                    let pathbuf = PathBuf::from(filename);

                    File::create(&pathbuf).unwrap();
                    self.open_file(pathbuf.clone())?;
                    self.file_paths.push(pathbuf);

                    break;
                }
                Input { key: Key::Esc, .. } => break,
                input => {
                    note_name_area.input(input);
                }
            }
        }

        Ok(())
    }

    fn insert_template(&mut self) -> Result<String, VaultError> {
        let mut template_name_area = TextArea::default();
        template_name_area.set_cursor_line_style(Style::default());
        template_name_area.set_block(Block::bordered().title("Note Search"));

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Length(3),
                    Constraint::Length(10),
                    Constraint::Min(1),
                ]
                .as_ref(),
            );

        let mut autocomplete_cursor = (0, 0);
        loop {
            let scores = {
                let mut scores: Vec<(String, i64)> = Vec::new();
                let lines = template_name_area.lines();

                let matcher = SkimMatcherV2::default();
                for file in &self.file_paths {
                    let to_match = &lines[0];
                    let matched = matcher.fuzzy_match(file.to_str().unwrap(), to_match);
                    if matched.is_some() {
                        scores.push((file.to_str().unwrap().to_string(), matched.unwrap()));
                    } else {
                        continue;
                    }
                }
                scores.sort();
                scores.reverse();
                scores
            };

            let mut autocomplete_area = TextArea::default();
            autocomplete_area.set_cursor_line_style(Style::default());
            autocomplete_area.set_block(Block::bordered());

            for (name, _) in scores {
                autocomplete_area.insert_str(name);
                autocomplete_area.insert_newline();
            }

            autocomplete_area.move_cursor(tui_textarea::CursorMove::Jump(
                autocomplete_cursor.0,
                autocomplete_cursor.1,
            ));

            self.terminal
                .draw(|frame| {
                    let chunks = layout.split(frame.area());

                    frame.render_widget(&template_name_area, chunks[0]);
                    frame.render_widget(&autocomplete_area, chunks[1]);
                    frame.render_widget(self.tabs[self.current_tab].textarea(), chunks[2]);
                })
                .unwrap();

            match Self::read()?.into() {
                Input {
                    key: Key::Enter, ..
                }
                | Input {
                    key: Key::Char('y'),
                    ctrl: true,
                    ..
                } => {
                    let (row, _) = autocomplete_area.cursor();
                    let lines = autocomplete_area.lines();

                    let pathbuf = PathBuf::from(&lines[row]);

                    self.file_paths.push(pathbuf.clone());

                    let template = Self::open_template(pathbuf.clone())?;
                    let tab = &mut self.tabs[self.current_tab];

                    let lines = template.textarea().lines();

                    for line in lines {
                        let mut line = line.to_string();
                        if line.contains("{{title}}") {
                            let inner = line.replace("{{title}}", tab.path().to_str().unwrap());
                            line = inner;
                        }

                        if line.contains("{{date:") {
                            let inner = get_formated_date(line.to_string());
                            line = inner;
                        }

                        tab.textareas[tab.current].insert_str(line);
                        tab.textareas[tab.current].insert_newline();
                    }

                    break;
                }
                Input { key: Key::Esc, .. } => break,
                input => {
                    if input
                        == (Input {
                            key: Key::Char('n'),
                            ctrl: true,
                            alt: false,
                            shift: false,
                        })
                        || input
                            == (Input {
                                key: Key::Char('p'),
                                ctrl: true,
                                alt: false,
                                shift: false,
                            })
                    {
                        autocomplete_area.input(input);
                        let (row, col) = autocomplete_area.cursor();
                        autocomplete_cursor = (row as u16, col as u16);
                    } else {
                        template_name_area.input(input);
                        autocomplete_cursor = (0, 0);
                    }
                }

            }
        }


        Ok("".to_string())
    }

    fn exec_command(&mut self, command: Command) -> Result<(), VaultError> {
        match command {
            Command::Quit => {
                self.tabs.remove(self.current_tab);
                if self.tabs.len() == 0 {
                    self.run = false;
                } else {
                    if self.current_tab >= self.tabs.len() && self.current_tab != 0 {
                        self.current_tab -= 1;
                    }
                }
            }
            Command::Save => {
                self.tabs[self.current_tab].save()?;
            }
            Command::SaveQuit => {
                self.tabs[self.current_tab].save()?;
                self.tabs.remove(self.current_tab);
                if self.tabs.len() == 0 {
                    self.run = false;
                } else {
                    if self.current_tab >= self.tabs.len() {
                        self.current_tab -= 1;
                    }
                }
            }
            Command::NewNote => {
                let result = self.new_note();
                self.handle_error(result);
            }
            Command::FollowLink => {
                let tab = &self.tabs[self.current_tab];
                let (row, col) = tab.textarea().cursor();
                let current_line = &tab.textarea().lines()[row];

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
                    let result = self.open_file(PathBuf::from(filename.to_string() + ".md"));
                    self.handle_error(result);
                }
            }
            Command::InsertTemplate => {
                let result = self.insert_template();
                self.handle_error(result);
            }
            Command::NewTab => {
                self.tabs.push(Editor::new());
                self.current_tab += 1;
            }
            Command::FocusTab(tab) => {
                let move_by: i32 = if tab == 0 { -1 } else { 1 };

                let tab = self.current_tab as i32 + move_by;

                if tab < 0 {
                    self.current_tab = 0;
                } else if tab >= self.tabs.len() as i32 {
                    self.current_tab = self.tabs.len() - 1;
                } else {
                    self.current_tab = tab as usize;
                }
            }
            Command::PreviousBuffer => {
                let tab = &mut self.tabs[self.current_tab];
                if tab.current != 0 {
                    tab.current -= 1;
                }
            }
            Command::NextBuffer => {
                let tab = &mut self.tabs[self.current_tab];
                if tab.current != tab.textareas.len() - 1 {
                    tab.current += 1;
                }
            }
            Command::SearchNote => {
                let inner_link = self.render_file_search()?;

                if inner_link == "" {
                    return Ok(());
                }

                let inner_link = &inner_link[0..inner_link.len()];

                let result = self.open_file(PathBuf::from(inner_link));
                self.handle_error(result);
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
        // TODO: When there is only one file in dir it opens the file and not the homepage.
        // Keep or not?
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

fn change_moment_syntax_to_chrono_syntax(moment: &str) -> &str {
    match moment {
        "M" => "%-m",
        "MM" => "%m",
        "MMM" => "%b",
        "MMMM" => "%B",
        "Y" => "%Y",
        "DD" => "%d",
        "D" => "%-d",
        "H" => "%-H",
        "HH" => "%H",
        "m" => "%-M",
        "mm" => "%M",
        _ => "",
    }
}

fn get_date(date: &str) -> String {
    let mut return_date = String::new();
    let mut current_format = String::new();
    let mut counter = 0;
    for char in date.to_string().chars() {
        counter += 1;
        let char = char.to_string();

        current_format += &char;

        let matched = change_moment_syntax_to_chrono_syntax(&current_format);

        if matched == "" {
            current_format.pop();
            return_date += change_moment_syntax_to_chrono_syntax(&current_format);
            current_format.clear();
            // This means there is no separator character between patterns
            // Like YMMDD instead of Y-MM-DD
            // So add it to current_format and do not add it to return_date
            if change_moment_syntax_to_chrono_syntax(&char) != "" {
                current_format += &char;
                continue;
            }

            return_date += &char;
        }

        if counter == date.len() {
            return_date += change_moment_syntax_to_chrono_syntax(&current_format);
        }
    }

    return_date
}

fn get_formated_date(string: String) -> String {
    let mut new_string_list: Vec<String> = Vec::new();

    // TODO: Dates cannot have spaces as I split the string by spaces
    for item in string.split(" ").map(|string| string.to_string()) {
        if item.contains("{{") && item.contains("}}") {
            let date_start = item.find("{{").unwrap();
            let date_end = item.find("}}").unwrap();

            // 2 for '{{' and 5 for 'date:'
            let date = &item[date_start + 7..date_end];
            let date = get_date(date);
            let date = Local::now().format(&date).to_string();

            let item_start = &item[0..date_start];
            // 2 for '}}
            let item_end = &item[date_end + 2..item.len()];

            new_string_list.push(item_start.to_string() + &date + item_end);
            continue;
        }

        new_string_list.push(item);
    }

    new_string_list.join(" ")
}
