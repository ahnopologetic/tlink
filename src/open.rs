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
        parts.get(i).filter(|s| !s.is_empty()).map(|s| s.to_string())
    };

    Ok(TmuxTarget { session: seg(0), window: seg(1), pane: seg(2) })
}

pub fn run(uri: &str) -> Result<()> {
    let target = parse_uri(uri)?;
    execute_switch(&target)?;

    if let Ok(config) = crate::config::load() {
        if let Some(name) = config.terminal {
            let _ = crate::terminal::from_name(&name).focus();
        }
    }
    Ok(())
}

fn execute_switch(target: &TmuxTarget) -> Result<()> {
    let Some(session) = &target.session else { return Ok(()) };

    // Build a fully-qualified tmux target: session[:window[.pane]]
    // A single switch-client call is correct — issuing separate select-window/select-pane
    // calls in subprocesses races against tmux's "current session" context and lands on
    // the wrong window.
    let tmux_target = match (&target.window, &target.pane) {
        (Some(w), Some(p)) => format!("{session}:{w}.{p}"),
        (Some(w), None) => format!("{session}:{w}"),
        _ => session.to_string(),
    };

    run_tmux(&["switch-client", "-t", &tmux_target])
}

fn run_tmux(args: &[&str]) -> Result<()> {
    let status = Command::new("tmux").args(args).status()?;
    if !status.success() {
        bail!("tmux {} exited with {}", args[0], status);
    }
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
        let t = TmuxTarget { session: Some("dorv".into()), window: None, pane: None };
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
        let t = TmuxTarget { session: Some("dorv".into()), window: Some("work".into()), pane: None };
        let target = format!("{}:{}", t.session.unwrap(), t.window.unwrap());
        assert_eq!(target, "dorv:work");
    }

    #[test]
    fn test_tmux_target_full() {
        let t = TmuxTarget { session: Some("dorv".into()), window: Some("work".into()), pane: Some("1".into()) };
        let target = format!("{}:{}.{}", t.session.unwrap(), t.window.unwrap(), t.pane.unwrap());
        assert_eq!(target, "dorv:work.1");
    }
}
