use anyhow::Result;
use std::process::Command;

pub struct TerminalAdapter {
    pub name: String,
}

pub fn from_name(name: &str) -> TerminalAdapter {
    TerminalAdapter {
        name: name.to_string(),
    }
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
                // wezterm CLI can spawn a command in a new tab
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
            _ => {
                // Ghostty and others: focus only — no public API for sending commands
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
