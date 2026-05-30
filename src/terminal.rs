use anyhow::Result;
use std::process::Command;

pub struct TerminalAdapter {
    pub name: String,
}

/// Map a raw `client_termtype` string from tmux to a canonical terminal name.
/// `client_termtype` is e.g. "ghostty 1.2.3", "Apple_Terminal", "iTerm.app".
pub fn from_termtype(termtype: &str) -> Option<String> {
    let lower = termtype.to_lowercase();
    if lower.starts_with("ghostty") {
        Some("Ghostty".into())
    } else if lower.starts_with("apple_terminal") {
        Some("Terminal.app".into())
    } else if lower.starts_with("iterm")
        || lower.starts_with("iterm2")
        || lower.starts_with("iterm.app")
    {
        Some("iTerm2".into())
    } else if lower.starts_with("wezterm") || lower.starts_with("wez") {
        Some("WezTerm".into())
    } else if lower.starts_with("kitty") {
        Some("Kitty".into())
    } else if lower.starts_with("alacritty") {
        Some("Alacritty".into())
    } else if lower.starts_with("warp") {
        Some("Warp".into())
    } else {
        None
    }
}

pub fn from_name(name: &str) -> TerminalAdapter {
    TerminalAdapter {
        name: name.to_string(),
    }
}

/// Try to detect the terminal emulator from an *attached* tmux client.
/// Reads `client_termtype` for the first client attached to any session.
/// Returns `None` if there are no attached clients.
pub fn detect_from_running_tmux() -> Option<TerminalAdapter> {
    let output = Command::new("tmux")
        .args(["list-clients", "-F", "#{client_termtype}"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            if let Some(name) = from_termtype(trimmed) {
                return Some(TerminalAdapter { name });
            }
        }
    }
    None
}

impl TerminalAdapter {
    pub fn focus(&self) -> Result<()> {
        // `tell application X to activate` is the reliable way to bring any
        // macOS app to the foreground. `open -a` only sends a launch event and
        // does not guarantee the window comes to front on a different Space.
        let app_name = match self.name.as_str() {
            "Kitty" => "kitty",
            other => other,
        };
        applescript_activate(app_name)
    }

    /// Tell the terminal to open a new window/tab running `tmux attach-session -t target`.
    /// Used when no tmux client is attached (truly detached), so switch-client won't work.
    pub fn attach_tmux(&self, target: &str) -> Result<()> {
        match self.name.as_str() {
            "iTerm2" => {
                let script = format!(
                    r#"tell application "iTerm2" to create window with default profile command "tmux attach-session -t {}""#,
                    target
                );
                Command::new("osascript").args(["-e", &script]).status()?;
            }
            "Terminal" | "Terminal.app" => {
                let script = format!(
                    r#"tell application "Terminal" to do script "tmux attach-session -t {}""#,
                    target
                );
                Command::new("osascript").args(["-e", &script]).status()?;
            }
            "WezTerm" => {
                Command::new("wezterm")
                    .args(["cli", "spawn", "--", "tmux", "attach-session", "-t", target])
                    .status()?;
            }
            "Kitty" => {
                Command::new("kitty")
                    .args([
                        "@",
                        "launch",
                        "--type=tab",
                        "--",
                        "tmux",
                        "attach-session",
                        "-t",
                        target,
                    ])
                    .status()?;
            }
            "Ghostty" => {
                // Ghostty has no AppleScript API for scripting new windows/tabs.
                // But it opens .command files as new tabs in the existing
                // window (or a new window if Ghostty isn't running).
                // env -u TMUX prevents tmux's "sessions should be nested" error.
                let tmp = std::env::temp_dir()
                    .join(format!("tlink-ghostty-{}.command", std::process::id()));
                let script = format!(
                    "#!/bin/sh\nenv -u TMUX tmux attach-session -t '{}'\nexec $SHELL\n",
                    target
                );
                if std::fs::write(&tmp, &script).is_ok() {
                    let _ = std::process::Command::new("chmod")
                        .arg("+x")
                        .arg(&tmp)
                        .status();
                    let _ = Command::new("open")
                        .args(["-a", "/Applications/Ghostty.app", &tmp.to_string_lossy()])
                        .status();
                }
            }
            _ => {
                // Unknown terminal: focus only
                self.focus()?;
            }
        }
        Ok(())
    }
}

fn applescript_activate(app_name: &str) -> Result<()> {
    let script = format!(r#"tell application "{app_name}" to activate"#);
    Command::new("osascript").args(["-e", &script]).status()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_name_stores_name() {
        let a = from_name("iTerm2");
        assert_eq!(a.name, "iTerm2");
    }

    #[test]
    fn test_from_name_unknown_does_not_panic() {
        let a = from_name("SomeFutureTerminal");
        assert_eq!(a.name, "SomeFutureTerminal");
    }

    #[test]
    fn test_terminal_app_alias_names() {
        let _ = from_name("Terminal");
        let _ = from_name("Terminal.app");
    }
}
