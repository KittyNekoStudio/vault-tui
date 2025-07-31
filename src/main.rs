mod editor;
mod homepage;
mod vault;
mod vim;

fn main() -> std::io::Result<()> {
    let mut vault = vault::Vault::new();
    let result = vault.run();
    ratatui::restore();
    result
}
