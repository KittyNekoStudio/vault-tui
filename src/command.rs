pub enum Command {
    Quit,
    Save,
    SaveQuit,
    Home,
    NewNote,
    FollowLink,
    InsertTemplate,
    NewTab,
    FocusTab(u8),
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
            "insert template" | "itm" => Command::InsertTemplate,
            "new tab" | "nt" => Command::NewTab,
            _ => Command::None,
        }
    }
}
