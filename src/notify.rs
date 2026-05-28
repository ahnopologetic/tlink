use anyhow::Result;
use serde::Deserialize;
use std::io::Read;
use std::process::Command;

#[derive(Deserialize, Default)]
struct Payload {
    notification_type: Option<String>,
    message: Option<String>,
}

fn type_to_title(t: &str) -> &'static str {
    match t {
        "idle_prompt"          => "Waiting for your input",
        "permission_prompt"    => "Permission needed",
        "auth_success"         => "Authenticated",
        "elicitation_dialog"   => "MCP: question for you",
        "elicitation_complete" => "MCP: dialog complete",
        "elicitation_response" => "MCP: response submitted",
        _                      => "Claude Code",
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

/// macOS bundle ID for known terminals — used to attribute notifications correctly.
fn bundle_id(terminal: &str) -> &'static str {
    match terminal {
        "Ghostty"                   => "com.mitchellh.ghostty",
        "iTerm2"                    => "com.googlecode.iterm2",
        "Terminal" | "Terminal.app" => "com.apple.Terminal",
        "Kitty"                     => "net.kovidgoyal.kitty",
        "WezTerm"                   => "com.github.wez.wezterm",
        _                           => "com.apple.Terminal",
    }
}

fn terminal_notifier_available() -> bool {
    Command::new("which")
        .arg("terminal-notifier")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn run(session: &str, window: &str, pane: &str) -> Result<()> {
    let mut stdin = String::new();
    std::io::stdin().read_to_string(&mut stdin)?;

    let payload: Payload = serde_json::from_str(&stdin).unwrap_or_default();
    let message = payload.message.as_deref().unwrap_or("Claude notification");
    let title   = payload.notification_type
        .as_deref()
        .map(type_to_title)
        .unwrap_or("Claude Code");

    let deeplink = format!("tmux://{}/{}/{}", session, window, pane);
    let location = format!("{} > {} > {}", session, window, pane);

    let config   = crate::config::load().unwrap_or_default();
    let method   = config.notification_method.as_deref().unwrap_or("osascript");
    let terminal = config.terminal.as_deref().unwrap_or("Ghostty");

    fire(method, terminal, title, message, &location, &deeplink)
}

fn fire(method: &str, terminal: &str, title: &str, message: &str, location: &str, deeplink: &str) -> Result<()> {
    match method {
        "terminal-notifier" => {
            fire_terminal_notifier(bundle_id(terminal), title, message, location, deeplink)?;
        }

        "dunstify" => {
            let cmd = format!(
                "ACTION=$(dunstify {t} {m} --action='default,Go there' \
                    --urgency=normal --icon=utilities-terminal --appname='Claude Code'); \
                 [ \"$ACTION\" = \"default\" ] && tlink open {dl}",
                t  = sh_quote(title),
                m  = sh_quote(message),
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
        // Prefer terminal-notifier if available — osascript's display notification
        // is owned by Script Editor, so clicking the notification opens Script Editor
        // instead of the user's terminal.
        _ => {
            if terminal_notifier_available() {
                fire_terminal_notifier(bundle_id(terminal), title, message, location, deeplink)?;
            } else {
                let script = format!(
                    "display notification \"{}\" with title \"{}\" subtitle \"{}\" sound name \"Glass\"",
                    applescript_escape(message),
                    applescript_escape(title),
                    applescript_escape(location),
                );
                Command::new("osascript").args(["-e", &script]).status()?;
            }
        }
    }
    Ok(())
}

fn fire_terminal_notifier(sender: &str, title: &str, message: &str, location: &str, deeplink: &str) -> Result<()> {
    Command::new("terminal-notifier")
        .args([
            "-title",    title,
            "-subtitle", location,
            "-message",  message,
            "-sender",   sender,
            "-execute",  &format!("tlink open {}", deeplink),
        ])
        .spawn()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_to_title_known_types() {
        assert_eq!(type_to_title("idle_prompt"),       "Waiting for your input");
        assert_eq!(type_to_title("permission_prompt"), "Permission needed");
        assert_eq!(type_to_title("auth_success"),      "Authenticated");
        assert_eq!(type_to_title("elicitation_dialog"), "MCP: question for you");
    }

    #[test]
    fn test_type_to_title_unknown_falls_back() {
        assert_eq!(type_to_title("unknown_type"), "Claude Code");
        assert_eq!(type_to_title(""),             "Claude Code");
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
