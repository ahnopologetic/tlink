use anyhow::{bail, Result};
use std::process::Command;
use std::time::Instant;

/// Simple stderr logger with timestamp. Use RUST_LOG=tlink=debug to enable.
/// Falls back to no-op when env var is unset or empty.
macro_rules! log {
    ($($arg:tt)*) => {{
        if std::env::var("RUST_LOG").unwrap_or_default().contains("tlink")
            || std::env::var("TLINK_LOG").is_ok()
        {{
            eprintln!("[tlink] {}", format_args!($($arg)*));
        }}
    }};
}

#[derive(Debug, PartialEq)]
pub struct TmuxTarget {
    pub session: Option<String>,
    pub window: Option<String>,
    pub pane: Option<String>,
    /// Terminal emulator from `?term=` query param in the URI
    pub term: Option<String>,
}

/// Simple percent-decode: only handles %XX hex sequences.
fn percent_decode(s: &str) -> String {
    let mut out = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16).unwrap_or(0) as u8;
            let lo = (bytes[i + 2] as char).to_digit(16).unwrap_or(0) as u8;
            out.push((hi << 4) | lo);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

pub fn parse_uri(uri: &str) -> Result<TmuxTarget> {
    let stripped = uri
        .strip_prefix("tmux://")
        .ok_or_else(|| anyhow::anyhow!("URI must start with tmux://, got: {uri}"))?;

    // Split off query parameter if present
    let (path_part, term) = if let Some(pos) = stripped.find('?') {
        let query = &stripped[pos + 1..];
        let path = &stripped[..pos];
        let t = query.strip_prefix("term=").map(percent_decode);
        (path, t)
    } else {
        (stripped, None)
    };

    let parts: Vec<&str> = path_part.splitn(3, '/').collect();
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
        term,
    })
}

/// Resolve a terminal adapter using the best available source:
/// 1. Terminal type from URI `?term=` query param (passed by new notifications)
/// 2. Detect from attached tmux client via `#{client_termtype}`
/// 3. Static config from `tlink setup`
fn resolve_adapter(target: &TmuxTarget) -> Option<crate::terminal::TerminalAdapter> {
    // Priority 1: terminal type embedded in the URI
    if let Some(ref term) = target.term {
        log!("resolve_adapter: trying term from URI: {term}");
        if let Some(name) = crate::terminal::from_termtype(term) {
            log!("resolve_adapter: URI term matched adapter: {name}");
            return Some(crate::terminal::from_name(&name));
        }
        // Even if we don't have a known adapter for it, try the raw name
        log!("resolve_adapter: URI term unknown, trying as raw app name: {term}");
        return Some(crate::terminal::from_name(term));
    }

    // Priority 2: detect from a running tmux client
    log!("resolve_adapter: trying detect_from_running_tmux()");
    if let Some(adapter) = crate::terminal::detect_from_running_tmux() {
        log!(
            "resolve_adapter: detected from tmux client: {}",
            adapter.name
        );
        return Some(adapter);
    }

    // Priority 3: static config
    let adapter = crate::config::load()
        .ok()
        .and_then(|c| c.terminal)
        .map(|name| crate::terminal::from_name(&name));
    log!(
        "resolve_adapter: config adapter={:?}",
        adapter.as_ref().map(|a| &a.name)
    );
    adapter
}

pub fn run(uri: &str) -> Result<()> {
    let _start = Instant::now();
    log!("open: uri={uri}");

    let target = parse_uri(uri)?;
    log!(
        "open: parsed session={:?} window={:?} pane={:?} term={:?}",
        target.session,
        target.window,
        target.pane,
        target.term
    );

    // Resolve terminal adapter from best available source.
    let adapter = resolve_adapter(&target);

    // Focus terminal FIRST so it is in front when tmux switch-client fires.
    // Without this, switch-client succeeds but the terminal stays hidden.
    if let Some(ref a) = adapter {
        log!("open: focusing terminal '{}'", a.name);
        let _ = a.focus();
        // Give the window manager time to actually bring the window to front.
        std::thread::sleep(std::time::Duration::from_millis(150));
    } else {
        log!("open: no terminal adapter resolved, skipping focus");
    }

    execute_switch(&target, adapter.as_ref())?;
    log!("open: completed in {:?}", _start.elapsed());
    Ok(())
}

fn execute_switch(
    target: &TmuxTarget,
    adapter: Option<&crate::terminal::TerminalAdapter>,
) -> Result<()> {
    let Some(session) = &target.session else {
        log!("execute_switch: no session in target, nothing to do");
        return Ok(());
    };

    let tmux_target = match (&target.window, &target.pane) {
        (Some(w), Some(p)) => format!("{session}:{w}.{p}"),
        (Some(w), None) => format!("{session}:{w}"),
        _ => session.to_string(),
    };
    log!("execute_switch: tmux_target={tmux_target}");

    // Ghostty: always open a new tab via .command file rather than using
    // switch-client (which silently moves sessions in invisible tabs).
    // For all other terminals: try switch-client first, fall back to attach_tmux.
    let is_ghostty = adapter.map(|a| a.name == "Ghostty").unwrap_or(false);

    if is_ghostty {
        log!("execute_switch: Ghostty — opening new tab via attach_tmux");
        if let Some(a) = adapter {
            let _ = a.attach_tmux(&tmux_target);
        }
        return Ok(());
    }

    // switch-client works when any tmux client is attached (even if the terminal
    // was backgrounded). If it fails the session is truly detached — fall back to
    // asking the terminal to run attach-session in a new window.
    log!("execute_switch: attempting `tmux switch-client -t {tmux_target}`");
    let switched = Command::new("tmux")
        .args(["switch-client", "-t", &tmux_target])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    log!("execute_switch: switch-client result={switched}");

    if !switched {
        log!("execute_switch: switch-client FAILED — no attached client");
        if let Some(a) = adapter {
            log!(
                "execute_switch: falling back to attach_tmux for '{}' target={}",
                a.name,
                tmux_target
            );
            let _ = a.attach_tmux(&tmux_target);
        } else {
            log!("execute_switch: no terminal adapter configured — bailing");
            bail!("tmux switch-client failed and no terminal adapter configured");
        }
        return Ok(()); // toast/flash require an attached client; skip for now
    }

    log!("execute_switch: switch-client SUCCEEDED — client is attached");

    // Status-bar toast.
    let label = match (&target.window, &target.pane) {
        (Some(w), Some(p)) => format!("tlink → {session}:{w}.{p}"),
        (Some(w), None) => format!("tlink → {session}:{w}"),
        _ => format!("tlink → {session}"),
    };
    log!("execute_switch: displaying toast '{label}'");
    let _ = Command::new("tmux")
        .args(["display-message", "-d", "2000", "-t", &tmux_target, &label])
        .status();

    // Flash the active pane border: set a vivid colour, then reset after 1.5 s.
    // pane-active-border-style is a window option, so target at window level.
    let win_target = match &target.window {
        Some(w) => format!("{session}:{w}"),
        None => session.to_string(),
    };
    log!("execute_switch: flashing border for {win_target}");
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
        assert!(t.term.is_none());
    }

    #[test]
    fn test_parse_session_and_window() {
        let t = parse_uri("tmux://mysession/2").unwrap();
        assert_eq!(t.session.as_deref(), Some("mysession"));
        assert_eq!(t.window.as_deref(), Some("2"));
        assert!(t.pane.is_none());
        assert!(t.term.is_none());
    }

    #[test]
    fn test_parse_full_uri() {
        let t = parse_uri("tmux://mysession/2/1").unwrap();
        assert_eq!(t.session.as_deref(), Some("mysession"));
        assert_eq!(t.window.as_deref(), Some("2"));
        assert_eq!(t.pane.as_deref(), Some("1"));
        assert!(t.term.is_none());
    }

    #[test]
    fn test_parse_empty_host() {
        let t = parse_uri("tmux://").unwrap();
        assert!(t.session.is_none());
        assert!(t.window.is_none());
        assert!(t.pane.is_none());
        assert!(t.term.is_none());
    }

    #[test]
    fn test_parse_invalid_scheme_errors() {
        assert!(parse_uri("https://foo").is_err());
        assert!(parse_uri("tmux:foo").is_err());
    }

    #[test]
    fn test_parse_with_term() {
        let t = parse_uri("tmux://mysession/0/0?term=ghostty").unwrap();
        assert_eq!(t.session.as_deref(), Some("mysession"));
        assert_eq!(t.term.as_deref(), Some("ghostty"));
    }

    #[test]
    fn test_parse_with_term_encoded() {
        let t = parse_uri("tmux://mysession/0/0?term=ghostty%201.2.3").unwrap();
        assert_eq!(t.session.as_deref(), Some("mysession"));
        assert_eq!(t.term.as_deref(), Some("ghostty 1.2.3"));
    }

    #[test]
    fn test_parse_term_only_session() {
        let t = parse_uri("tmux://mysession?term=Apple_Terminal").unwrap();
        assert_eq!(t.session.as_deref(), Some("mysession"));
        assert_eq!(t.term.as_deref(), Some("Apple_Terminal"));
    }

    #[test]
    fn test_tmux_target_session_only() {
        let t = TmuxTarget {
            session: Some("dorv".into()),
            window: None,
            pane: None,
            term: None,
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
            term: None,
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
            term: None,
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
