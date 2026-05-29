use anyhow::Result;
use serde::Deserialize;
use std::io::Read;
use std::process::Command;

#[derive(Deserialize, Default)]
struct Payload {
    hook_event_name: Option<String>,
    // Notification
    notification_type: Option<String>,
    message: Option<String>,
    // StopFailure
    error_type: Option<String>,
    // Tool events
    tool_name: Option<String>,
    // Agent events
    agent_type: Option<String>,
    // Task events
    task_title: Option<String>,
    // Session events
    reason: Option<String>,
}

fn type_to_title(t: &str) -> &'static str {
    match t {
        "idle_prompt" => "Waiting for your input",
        "permission_prompt" => "Permission needed",
        "auth_success" => "Authenticated",
        "elicitation_dialog" => "MCP: question for you",
        "elicitation_complete" => "MCP: dialog complete",
        "elicitation_response" => "MCP: response submitted",
        _ => "Claude Code",
    }
}

/// Wrap a string in single quotes, escaping any interior single quotes.
fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Escape a string for use inside an AppleScript double-quoted string.
fn applescript_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn alerter_available() -> bool {
    Command::new("which")
        .arg("alerter")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn resolve(payload: &Payload) -> (String, String) {
    match payload.hook_event_name.as_deref().unwrap_or("Notification") {
        "Notification" => {
            let t = payload
                .notification_type
                .as_deref()
                .map(type_to_title)
                .unwrap_or("Claude Code");
            let m = payload
                .message
                .clone()
                .unwrap_or_else(|| "Claude notification".into());
            (t.into(), m)
        }
        "Stop" => (
            "Claude finished".into(),
            "Claude finished responding and is waiting for your input.".into(),
        ),
        "StopFailure" => (
            "Claude error".into(),
            format!(
                "Turn failed: {}",
                payload.error_type.as_deref().unwrap_or("unknown error")
            ),
        ),
        "PostToolUse" => (
            "Tool completed".into(),
            format!(
                "{} finished",
                payload.tool_name.as_deref().unwrap_or("Tool")
            ),
        ),
        "PostToolUseFailure" => (
            "Tool failed".into(),
            format!("{} error", payload.tool_name.as_deref().unwrap_or("Tool")),
        ),
        "SubagentStop" => (
            "Subagent done".into(),
            format!(
                "{} subagent finished",
                payload.agent_type.as_deref().unwrap_or("A")
            ),
        ),
        "TeammateIdle" => (
            "Teammate idle".into(),
            format!(
                "{} is waiting for your input",
                payload.agent_type.as_deref().unwrap_or("Teammate")
            ),
        ),
        "TaskCreated" => (
            "Task created".into(),
            payload
                .task_title
                .clone()
                .unwrap_or_else(|| "New task".into()),
        ),
        "TaskCompleted" => (
            "Task complete".into(),
            "A task was marked as completed.".into(),
        ),
        "SessionStart" => (
            "Session started".into(),
            "A Claude Code session has started.".into(),
        ),
        "SessionEnd" => (
            "Session ended".into(),
            format!(
                "Session ended: {}",
                payload.reason.as_deref().unwrap_or("unknown")
            ),
        ),
        other => ("Claude Code".into(), format!("{} event", other)),
    }
}

pub fn run(session: &str, window: &str, pane: &str) -> Result<()> {
    let mut stdin = String::new();
    std::io::stdin().read_to_string(&mut stdin)?;

    let payload: Payload = serde_json::from_str(&stdin).unwrap_or_default();
    let (title, message) = resolve(&payload);

    let deeplink = format!("tmux://{}/{}/{}", session, window, pane);
    let location = format!("{} > {} > {}", session, window, pane);

    let config = crate::config::load().unwrap_or_default();
    let method = config.notification_method.as_deref().unwrap_or("osascript");

    fire(method, &title, &message, &location, &deeplink)
}

fn fire(method: &str, title: &str, message: &str, location: &str, deeplink: &str) -> Result<()> {
    match method {
        "alerter" => {
            fire_alerter(title, message, location, deeplink)?;
        }

        "terminal-notifier" => {
            fire_terminal_notifier(title, message, location, deeplink)?;
        }

        "dunstify" => {
            let cmd = format!(
                "ACTION=$(dunstify {t} {m} --action='default,Go there' \
                    --urgency=normal --icon=utilities-terminal --appname='Claude Code'); \
                 [ \"$ACTION\" = \"default\" ] && tlink open {dl}",
                t = sh_quote(title),
                m = sh_quote(message),
                dl = sh_quote(deeplink),
            );
            Command::new("sh").args(["-c", &cmd]).spawn()?;
        }

        "notify-send" => {
            Command::new("notify-send")
                .args([
                    title,
                    &format!("{}\n{}", message, location),
                    "--urgency=normal",
                    "--icon=utilities-terminal",
                    "--app-name=Claude Code",
                ])
                .status()?;
        }

        // "osascript" or any unknown value:
        // alerter is the preferred macOS fallback — UNUserNotificationCenter with real click
        // callbacks. terminal-notifier's -execute/-open are broken on macOS 12+.
        // Last resort: osascript display notification (no click callback) + open location
        // to invoke the URL scheme immediately when the notification fires.
        _ => {
            if alerter_available() {
                fire_alerter(title, message, location, deeplink)?;
            } else {
                let script = format!(
                    "display notification \"{}\" with title \"{}\" subtitle \"{}\" sound name \"Glass\"\n\
                     open location \"{}\"",
                    applescript_escape(message),
                    applescript_escape(title),
                    applescript_escape(location),
                    applescript_escape(deeplink),
                );
                Command::new("osascript").args(["-e", &script]).status()?;
            }
        }
    }
    Ok(())
}

fn fire_alerter(title: &str, message: &str, location: &str, deeplink: &str) -> Result<()> {
    // alerter blocks until user interaction; run in a background shell that opens
    // the deeplink when the notification body (@CONTENTCLICKED) or action button
    // (@ACTIONCLICKED) is clicked.
    let cmd = format!(
        "result=$(alerter --title {t} --subtitle {loc} --message {m} \
            --actions 'Open' --close-label 'Dismiss' --sound 'Glass' --timeout 60); \
         case \"$result\" in @CONTENTCLICKED|@ACTIONCLICKED) open {dl} ;; esac",
        t = sh_quote(title),
        loc = sh_quote(location),
        m = sh_quote(message),
        dl = sh_quote(deeplink),
    );
    Command::new("sh").args(["-c", &cmd]).spawn()?;
    Ok(())
}

fn fire_terminal_notifier(
    title: &str,
    message: &str,
    location: &str,
    deeplink: &str,
) -> Result<()> {
    Command::new("terminal-notifier")
        .args([
            "-title",
            title,
            "-subtitle",
            location,
            "-message",
            message,
            // -execute is broken on macOS 12+ (command never fires).
            // -open invokes the registered OS URL scheme handler on click,
            // which routes through tlink's tmux:// handler without PATH issues.
            "-open",
            deeplink,
        ])
        .spawn()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_to_title_known_types() {
        assert_eq!(type_to_title("idle_prompt"), "Waiting for your input");
        assert_eq!(type_to_title("permission_prompt"), "Permission needed");
        assert_eq!(type_to_title("auth_success"), "Authenticated");
        assert_eq!(type_to_title("elicitation_dialog"), "MCP: question for you");
    }

    #[test]
    fn test_type_to_title_unknown_falls_back() {
        assert_eq!(type_to_title("unknown_type"), "Claude Code");
        assert_eq!(type_to_title(""), "Claude Code");
    }

    #[test]
    fn test_sh_quote_plain() {
        assert_eq!(sh_quote("hello world"), "'hello world'");
    }

    #[test]
    fn test_sh_quote_with_single_quotes() {
        assert_eq!(sh_quote("it's fine"), r"'it'\''s fine'");
    }

    #[test]
    fn test_applescript_escape_quotes() {
        assert_eq!(applescript_escape(r#"say "hi""#), r#"say \"hi\""#);
    }

    #[test]
    fn test_payload_deserializes_message_and_type() {
        let json = r#"{"notification_type":"idle_prompt","message":"Done!"}"#;
        let p: Payload = serde_json::from_str(json).unwrap();
        assert_eq!(p.notification_type.as_deref(), Some("idle_prompt"));
        assert_eq!(p.message.as_deref(), Some("Done!"));
    }

    #[test]
    fn test_payload_missing_fields_default() {
        let p: Payload = serde_json::from_str("{}").unwrap_or_default();
        assert!(p.notification_type.is_none());
        assert!(p.message.is_none());
    }
}
