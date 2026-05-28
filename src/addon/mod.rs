pub mod claude_notification;

use anyhow::{bail, Result};

pub struct AddonInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub installed: bool,
}

fn registry() -> Vec<AddonInfo> {
    vec![AddonInfo {
        name: "claude-notification",
        description: "Native desktop notification when Claude calls; click to navigate back to that tmux pane",
        installed: claude_notification::is_installed(),
    }]
}

pub fn install(name: &str) -> Result<()> {
    match name {
        "claude-notification" => claude_notification::install(),
        _ => bail!("unknown add-on '{name}'. Run `tlink list add-ons` to see available add-ons."),
    }
}

pub fn delete(name: &str) -> Result<()> {
    match name {
        "claude-notification" => claude_notification::uninstall(),
        _ => bail!("unknown add-on '{name}'."),
    }
}

pub fn list() -> Result<()> {
    let addons = registry();
    println!("{:<25} {:<15} {}", "NAME", "STATUS", "DESCRIPTION");
    println!("{}", "─".repeat(80));
    for a in &addons {
        let status = if a.installed { "installed" } else { "not installed" };
        println!("{:<25} {:<15} {}", a.name, status, a.description);
    }
    Ok(())
}
