mod vim;
mod editor;
mod vault;
mod homepage;

fn main() -> std::io::Result<()> {
    let mut vault = vault::Vault::new();
    let result = vault.run();
    ratatui::restore();
    result
}
