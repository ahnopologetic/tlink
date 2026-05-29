use anyhow::Result;
use std::process::Command;

const LSREGISTER: &str = "/System/Library/Frameworks/CoreServices.framework/Versions/A/Frameworks/LaunchServices.framework/Versions/A/Support/lsregister";

pub struct StatusInfo {
    pub bundle_exists: bool,
    pub scheme_registered: bool,
    pub configured_terminal: Option<String>,
    pub tmux_running: bool,
    pub sessions: Vec<String>,
}

pub fn collect() -> StatusInfo {
    let bundle_exists = crate::bundle::bundle_path().exists();

    let scheme_registered = bundle_exists
        && Command::new(LSREGISTER)
            .args(["-dump"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains("tmux"))
            .unwrap_or(false);

    let configured_terminal = crate::config::load().ok().and_then(|c| c.terminal);

    let tmux_out = Command::new("tmux").args(["list-sessions"]).output();
    let (tmux_running, sessions) = match tmux_out {
        Ok(out) if out.status.success() => {
            let lines = String::from_utf8_lossy(&out.stdout)
                .lines()
                .map(|l| l.to_string())
                .collect();
            (true, lines)
        }
        _ => (false, vec![]),
    };

    StatusInfo {
        bundle_exists,
        scheme_registered,
        configured_terminal,
        tmux_running,
        sessions,
    }
}

pub fn run() -> Result<()> {
    let s = collect();
    println!(
        "URI scheme (tmux://): {}",
        if s.scheme_registered {
            "registered"
        } else {
            "not registered"
        }
    );
    println!(
        "TmuxLink.app:         {}",
        if s.bundle_exists {
            "present"
        } else {
            "missing"
        }
    );
    println!(
        "Terminal:             {}",
        s.configured_terminal.as_deref().unwrap_or("not configured")
    );
    println!(
        "tmux server:          {}",
        if s.tmux_running {
            "running"
        } else {
            "not running"
        }
    );
    if s.tmux_running {
        if s.sessions.is_empty() {
            println!("Sessions:             (none)");
        } else {
            println!("Sessions:");
            for session in &s.sessions {
                println!("  {}", session);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_does_not_panic() {
        let _ = collect();
    }
}
