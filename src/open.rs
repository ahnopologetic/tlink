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
    run_tmux(&["switch-client", "-t", session])?;
    let Some(window) = &target.window else { return Ok(()) };
    run_tmux(&["select-window", "-t", window])?;
    let Some(pane) = &target.pane else { return Ok(()) };
    run_tmux(&["select-pane", "-t", pane])
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
}
