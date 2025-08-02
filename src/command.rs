pub enum Command {
    Quit,
    Save,
    SaveQuit,
    NewNote,
    FollowLink,
    PreviousBuf,
    None,
}

impl Command {
    pub fn str_to_command(string: &str) -> Self {
        match string {
            "q" | "quit" => Command::Quit,
            "w" | "write" | "save" => Command::Save,
            "wq" => Command::SaveQuit,
            "nn" | "new note" => Command::NewNote,
            "follow" | "follow link" | "fl" => Command::FollowLink,
            "previous buf" | "prevb" | "pb" => Command::PreviousBuf,
            _ => Command::None,
        }
    }
}
