pub mod claude_notification;
pub mod codex_notification;
pub mod gemini_notification;
mod interactive;
pub mod pi_notification;

use anyhow::{bail, Result};

pub struct AddonInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub installed: bool,
}

pub(crate) fn registry() -> Vec<AddonInfo> {
    vec![
        AddonInfo {
            name: "claude-notification",
            description: "Native desktop notification when Claude calls; click to navigate back to that tmux pane",
            installed: claude_notification::is_installed(),
        },
        AddonInfo {
            name: "codex-notification",
            description: "Native desktop notification when Codex CLI finishes; click to navigate back to that tmux pane",
            installed: codex_notification::is_installed(),
        },
        AddonInfo {
            name: "gemini-notification",
            description: "Native desktop notification when Gemini CLI finishes; click to navigate back to that tmux pane",
            installed: gemini_notification::is_installed(),
        },
        AddonInfo {
            name: "pi-notification",
            description: "Native desktop notification when Pi agent events fire; click to navigate back to that tmux pane",
            installed: pi_notification::is_installed(),
        },
    ]
}

pub fn install(name: &str) -> Result<()> {
    match name {
        "claude-notification" => claude_notification::install(),
        "codex-notification" => codex_notification::install(),
        "gemini-notification" => gemini_notification::install(),
        "pi-notification" => pi_notification::install(),
        _ => bail!("unknown add-on '{name}'. Run `tlink list add-ons` to see available add-ons."),
    }
}

pub fn delete(name: &str) -> Result<()> {
    match name {
        "claude-notification" => claude_notification::uninstall(),
        "codex-notification" => codex_notification::uninstall(),
        "gemini-notification" => gemini_notification::uninstall(),
        "pi-notification" => pi_notification::uninstall(),
        _ => bail!("unknown add-on '{name}'."),
    }
}

pub fn list() -> Result<()> {
    let addons = registry();
    println!("{:<25} {:<15} DESCRIPTION", "NAME", "STATUS");
    println!("{}", "─".repeat(80));
    for a in &addons {
        let status = if a.installed {
            "installed"
        } else {
            "not installed"
        };
        println!("{:<25} {:<15} {}", a.name, status, a.description);
    }
    Ok(())
}

/// Interactive add-on selector using ratatui.
pub fn install_interactive() -> Result<()> {
    interactive::run()
}
