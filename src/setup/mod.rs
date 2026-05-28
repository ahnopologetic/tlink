mod wizard;

use anyhow::Result;

pub fn run() -> Result<()> {
    match wizard::run_wizard()? {
        Some(terminal) => {
            println!("Setup complete! Terminal: {terminal}");
            println!("Run `tlink status` to verify, or `open tmux://session` to test.");
        }
        None => {
            println!("Setup cancelled.");
        }
    }
    Ok(())
}
