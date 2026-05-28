use anyhow::Result;

pub fn run() -> Result<()> {
    println!("Removing ~/Applications/TmuxLink.app...");
    crate::bundle::remove()?;
    println!("Rebuilding and registering TmuxLink.app...");
    crate::bundle::create()?;
    println!("Done. tmux:// URI scheme re-registered.");
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_restart_compiles() {
        assert!(true);
    }
}
