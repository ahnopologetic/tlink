use anyhow::{bail, Result};
use std::process::Command;

#[derive(Debug, PartialEq)]
pub struct TmuxTarget {
    pub session: Option<String>,
    pub window: Option<String>,
    pub pane: Option<String>,
}

pub fn parse_uri(uri: &str) -> Result<TmuxTarget> {
    let stripped = uri
        .strip_prefix("tmux://")
        .ok_or_else(|| anyhow::anyhow!("URI must start with tmux://, got: {uri}"))?;

    let parts: Vec<&str> = stripped.splitn(3, '/').collect();
    let seg = |i: usize| -> Option<String> {
        parts
            .get(i)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
    };

    Ok(TmuxTarget {
        session: seg(0),
        window: seg(1),
        pane: seg(2),
    })
}

pub fn run(uri: &str) -> Result<()> {
    let target = parse_uri(uri)?;

    // Load terminal adapter once — used for both focus and attach fallback.
    let adapter = crate::config::load()
        .ok()
        .and_then(|c| c.terminal)
        .map(|name| crate::terminal::from_name(&name));

    // Focus terminal FIRST so it is in front when tmux switch-client fires.
    // Without this, switch-client succeeds but the terminal stays hidden.
    if let Some(ref a) = adapter {
        let _ = a.focus();
        // Give the window manager time to actually bring the window to front.
        std::thread::sleep(std::time::Duration::from_millis(150));
    }

    execute_switch(&target, adapter.as_ref())?;
    Ok(())
}

fn execute_switch(
    target: &TmuxTarget,
    adapter: Option<&crate::terminal::TerminalAdapter>,
) -> Result<()> {
    let Some(session) = &target.session else {
        return Ok(());
    };

    let tmux_target = match (&target.window, &target.pane) {
        (Some(w), Some(p)) => format!("{session}:{w}.{p}"),
        (Some(w), None) => format!("{session}:{w}"),
        _ => session.to_string(),
    };

    // switch-client works when any tmux client is attached (even if the terminal
    // was backgrounded). If it fails the session is truly detached — fall back to
    // asking the terminal to run attach-session in a new window.
    let switched = Command::new("tmux")
        .args(["switch-client", "-t", &tmux_target])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !switched {
        if let Some(a) = adapter {
            let _ = a.attach_tmux(&tmux_target);
        } else {
            bail!("tmux switch-client failed and no terminal adapter configured");
        }
        return Ok(()); // toast/flash require an attached client; skip for now
    }

    // Status-bar toast.
    let label = match (&target.window, &target.pane) {
        (Some(w), Some(p)) => format!("tlink → {session}:{w}.{p}"),
        (Some(w), None) => format!("tlink → {session}:{w}"),
        _ => format!("tlink → {session}"),
    };
    let _ = Command::new("tmux")
        .args(["display-message", "-d", "2000", "-t", &tmux_target, &label])
        .status();

    // Flash the active pane border: set a vivid colour, then reset after 1.5 s.
    // pane-active-border-style is a window option, so target at window level.
    let win_target = match &target.window {
        Some(w) => format!("{session}:{w}"),
        None => session.to_string(),
    };
    let _ = Command::new("tmux")
        .args([
            "set-option",
            "-t",
            &win_target,
            "pane-active-border-style",
            "fg=colour46,bold",
        ])
        .status();
    let reset = format!(
        "sleep 1.5 && tmux set-option -ut '{}' pane-active-border-style",
        win_target
    );
    let _ = Command::new("sh").args(["-c", &reset]).spawn();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_session_only() {
        let t = parse_uri("tmux://mysession").unwrap();
        assert_eq!(t.session.as_deref(), Some("mysession"));
        assert!(t.window.is_none());
        assert!(t.pane.is_none());
    }

    #[test]
    fn test_parse_session_and_window() {
        let t = parse_uri("tmux://mysession/2").unwrap();
        assert_eq!(t.session.as_deref(), Some("mysession"));
        assert_eq!(t.window.as_deref(), Some("2"));
        assert!(t.pane.is_none());
    }

    #[test]
    fn test_parse_full_uri() {
        let t = parse_uri("tmux://mysession/2/1").unwrap();
        assert_eq!(t.session.as_deref(), Some("mysession"));
        assert_eq!(t.window.as_deref(), Some("2"));
        assert_eq!(t.pane.as_deref(), Some("1"));
    }

    #[test]
    fn test_parse_empty_host() {
        let t = parse_uri("tmux://").unwrap();
        assert!(t.session.is_none());
        assert!(t.window.is_none());
        assert!(t.pane.is_none());
    }

    #[test]
    fn test_parse_invalid_scheme_errors() {
        assert!(parse_uri("https://foo").is_err());
        assert!(parse_uri("tmux:foo").is_err());
    }

    #[test]
    fn test_tmux_target_session_only() {
        let t = TmuxTarget {
            session: Some("dorv".into()),
            window: None,
            pane: None,
        };
        // single switch-client to session
        assert_eq!(
            match (&t.window, &t.pane) {
                (Some(w), Some(p)) => format!("{}:{}.{}", t.session.as_ref().unwrap(), w, p),
                (Some(w), None) => format!("{}:{}", t.session.as_ref().unwrap(), w),
                _ => t.session.unwrap(),
            },
            "dorv"
        );
    }

    #[test]
    fn test_tmux_target_session_window() {
        let t = TmuxTarget {
            session: Some("dorv".into()),
            window: Some("work".into()),
            pane: None,
        };
        let target = format!("{}:{}", t.session.unwrap(), t.window.unwrap());
        assert_eq!(target, "dorv:work");
    }

    #[test]
    fn test_tmux_target_full() {
        let t = TmuxTarget {
            session: Some("dorv".into()),
            window: Some("work".into()),
            pane: Some("1".into()),
        };
        let target = format!(
            "{}:{}.{}",
            t.session.unwrap(),
            t.window.unwrap(),
            t.pane.unwrap()
        );
        assert_eq!(target, "dorv:work.1");
    }
}
