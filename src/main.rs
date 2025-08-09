mod command;
mod editor;
mod error;
mod vault;
mod vim;

fn main() {
    let mut vault = vault::Vault::new();
    vault.run();
    ratatui::restore();
}
