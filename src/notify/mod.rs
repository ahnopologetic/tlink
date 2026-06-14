mod adapter;
mod dunstify;
pub mod icon;
mod notify_send;
mod osascript;
mod terminal_notifier;
pub mod utils;

pub use adapter::{NotificationAdapter, NotificationRequest};

use anyhow::Result;
use std::io::Read;

// ── Hook payload adapter (per-agent resolution) ──────────────────────────────

mod hooks;
use hooks::HookPayload;

/// RFC 3986 unreserved-only percent-encoding.
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push_str(&format!("%{:02X}", b));
            }
        }
    }
    out
}

fn build_deeplink(session: &str, window: &str, pane: &str, term: &str) -> String {
    let s = percent_encode(session);
    let w = percent_encode(window);
    let p = percent_encode(pane);
    if term.is_empty() {
        format!("tmux://{}/{}/{}", s, w, p)
    } else {
        let term_name = term.split_whitespace().next().unwrap_or(term);
        format!(
            "tmux://{}/{}/{}?term={}",
            s,
            w,
            p,
            percent_encode(term_name)
        )
    }
}

/// Run a notification: read hook JSON from stdin, resolve via the correct
/// agent adapter, and fire a desktop notification.
pub fn run(session: &str, window: &str, pane: &str, term: &str, source: &str) -> Result<()> {
    let mut stdin = String::new();
    std::io::stdin().read_to_string(&mut stdin)?;

    let mut payload: HookPayload = serde_json::from_str(&stdin).unwrap_or_default();
    // CLI source overrides any embedded source (the hook script's --source flag
    // is authoritative for backward compatibility with unmodified hook payloads)
    if !source.is_empty() && payload.source.is_none() {
        payload.source = Some(source.to_string());
    }
    let (title, message, _choices) = hooks::resolve(&payload);

    let deeplink = build_deeplink(session, window, pane, term);
    let location = format!("{} > {} > {}", session, window, pane);

    let icon_path = icon::ensure_icon()
        .unwrap_or_else(|_| {
            // If we can't write the icon to disk, fall back to an empty path.
            // The adapters will skip icon usage when the path is empty.
            std::path::PathBuf::new()
        })
        .to_string_lossy()
        .to_string();

    let config = crate::config::load().unwrap_or_default();
    let default_method = if cfg!(target_os = "macos") {
        "terminal-notifier"
    } else {
        "dunstify"
    };
    let method = config
        .notification_method
        .as_deref()
        .unwrap_or(default_method);

    let req = NotificationRequest {
        title,
        message,
        location,
        deeplink,
        session: session.to_string(),
        icon_path,
    };

    make_adapter(method).notify(&req)
}

pub fn make_adapter(method: &str) -> Box<dyn NotificationAdapter> {
    match method {
        "terminal-notifier" => Box::new(terminal_notifier::TerminalNotifierAdapter),
        "dunstify" => Box::new(dunstify::DunstifyAdapter),
        "notify-send" => Box::new(notify_send::NotifySendAdapter),
        _ => Box::new(osascript::OsascriptAdapter),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── make_adapter ──────────────────────────────────────────────────────────

    #[test]
    fn factory_terminal_notifier_method() {
        assert_eq!(
            make_adapter("terminal-notifier").name(),
            "terminal-notifier"
        );
    }

    #[test]
    fn factory_dunstify_method() {
        assert_eq!(make_adapter("dunstify").name(), "dunstify");
    }

    #[test]
    fn factory_notify_send_method() {
        assert_eq!(make_adapter("notify-send").name(), "notify-send");
    }

    #[test]
    fn factory_osascript_method() {
        assert_eq!(make_adapter("osascript").name(), "osascript");
    }

    #[test]
    fn factory_unknown_method_falls_back_to_osascript() {
        assert_eq!(make_adapter("xdg-desktop-portal").name(), "osascript");
    }

    // ── build_deeplink ────────────────────────────────────────────────────────

    #[test]
    fn deeplink_plain_session() {
        let uri = build_deeplink("mysession", "0", "1", "");
        assert_eq!(uri, "tmux://mysession/0/1");
    }

    #[test]
    fn deeplink_encodes_slash_in_session() {
        let uri = build_deeplink("work/backend", "0", "1", "");
        assert_eq!(uri, "tmux://work%2Fbackend/0/1");
    }

    #[test]
    fn deeplink_encodes_slash_in_window() {
        let uri = build_deeplink("s", "win/name", "0", "");
        assert_eq!(uri, "tmux://s/win%2Fname/0");
    }

    #[test]
    fn deeplink_encodes_space_in_session() {
        let uri = build_deeplink("my session", "0", "0", "");
        assert_eq!(uri, "tmux://my%20session/0/0");
    }

    #[test]
    fn deeplink_with_term_encodes_segments_and_term() {
        let uri = build_deeplink("work/backend", "0", "0", "ghostty 1.2.3");
        assert_eq!(uri, "tmux://work%2Fbackend/0/0?term=ghostty");
    }
}
