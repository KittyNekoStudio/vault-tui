pub enum Command {
    Quit,
    Save,
    SaveQuit,
    Home,
    NewNote,
    FollowLink,
    PreviousBuf,
    None,
}

impl Command {
    pub fn str_to_command(string: &str) -> Self {
        match string {
            "quit" | "q" => Command::Quit,
            "write" | "w" | "save" => Command::Save,
            "wq" => Command::SaveQuit,
            "home" | "h" => Command::Home,
            "new note" | "nn" => Command::NewNote,
            "follow" | "follow link" | "fl" => Command::FollowLink,
            "previous buf" | "prevb" | "pb" => Command::PreviousBuf,
            _ => Command::None,
        }
    }
}
