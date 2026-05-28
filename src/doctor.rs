use anyhow::Result;
use std::process::Command;

const LSREGISTER: &str = "/System/Library/Frameworks/CoreServices.framework/Versions/A/Frameworks/LaunchServices.framework/Versions/A/Support/lsregister";

struct Check {
    name: &'static str,
    pass: bool,
    advisory: bool,
}

pub fn run() -> Result<()> {
    let checks = vec![
        Check { name: "tmux binary in PATH", pass: tmux_in_path(), advisory: false },
        Check { name: "tmux server running", pass: tmux_server_running(), advisory: false },
        Check {
            name: "~/Applications/TmuxLink.app exists",
            pass: crate::bundle::bundle_path().exists(),
            advisory: false,
        },
        Check { name: "tmux:// scheme in lsregister", pass: scheme_in_lsregister(), advisory: false },
        Check {
            name: "configured terminal app exists in /Applications",
            pass: configured_terminal_exists(),
            advisory: false,
        },
        Check {
            name: "config file exists (~/.config/tlink/config.toml)",
            pass: crate::config::config_path().exists(),
            advisory: false,
        },
    ];

    let mut any_failed = false;
    for check in &checks {
        let icon = if check.pass { "✓" } else { "✗" };
        let advisory = if !check.pass && check.advisory { " (advisory)" } else { "" };
        println!("{icon} {}{advisory}", check.name);
        if !check.pass && !check.advisory {
            any_failed = true;
        }
    }

    if any_failed {
        eprintln!("\nOne or more checks failed. Run `tlink setup` to fix.");
        std::process::exit(1);
    }
    Ok(())
}

pub fn tmux_in_path() -> bool {
    Command::new("which").arg("tmux").status().map(|s| s.success()).unwrap_or(false)
}

pub fn tmux_server_running() -> bool {
    Command::new("tmux")
        .arg("list-sessions")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn scheme_in_lsregister() -> bool {
    Command::new(LSREGISTER)
        .args(["-dump"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("tmux://"))
        .unwrap_or(false)
}

pub fn configured_terminal_exists() -> bool {
    crate::config::load()
        .ok()
        .and_then(|c| c.terminal)
        .map(|t| std::path::Path::new(&format!("/Applications/{}.app", t)).exists())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tmux_in_path_returns_bool() {
        let _ = tmux_in_path();
    }

    #[test]
    fn test_tmux_server_check_returns_bool() {
        let _ = tmux_server_running();
    }

    #[test]
    fn test_scheme_in_lsregister_returns_bool() {
        let _ = scheme_in_lsregister();
    }

    #[test]
    fn test_configured_terminal_exists_returns_bool() {
        let _ = configured_terminal_exists();
    }
}
