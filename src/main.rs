mod command;
mod editor;
mod vault;
mod vim;

fn main() -> std::io::Result<()> {
    let mut vault = vault::Vault::new();
    let result = vault.run();
    ratatui::restore();
    result
}
