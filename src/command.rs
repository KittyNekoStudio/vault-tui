pub enum Command {
    Quit,
    Save,
    SaveQuit,
    NewNote,
    None,
}

impl Command {
    pub fn str_to_command(string: &str) -> Self {
        match string {
            "q" | "quit" => Command::Quit,
            "w" | "write" | "save" => Command::Save,
            "wq" => Command::SaveQuit,
            "nn" | "new note" => Command::NewNote,
            _ => Command::None,
        }
    }
}
