use anyhow::{Context, Result};
use std::path::PathBuf;

// Hook stdin JSON (from Claude Code Notification event):
//   { "session_id", "transcript_path", "cwd", "hook_event_name",
//     "notification_type", "message" }
const HOOK_SCRIPT: &str = r##"#!/bin/bash
# tlink claude-notification hook — invoked by Claude Code on Notification events

SESSION=$(tmux display-message -p "#{session_name}" 2>/dev/null) || exit 0
WINDOW=$(tmux display-message -p "#{window_name}" 2>/dev/null) || exit 0
PANE=$(tmux display-message -p "#{pane_index}" 2>/dev/null) || exit 0
[ -z "$SESSION" ] && exit 0

INPUT=$(cat)
MESSAGE=$(printf '%s' "$INPUT" | python3 -c "
import sys, json
try:
    d = json.loads(sys.stdin.read())
    print(d.get('message', 'Claude notification'))
except Exception:
    print('Claude notification')
" 2>/dev/null || echo "Claude notification")

DEEPLINK="tmux://${SESSION}/${WINDOW}/${PANE}"
LOCATION="${SESSION}:${WINDOW}.${PANE}"

if [[ "$(uname -s)" == "Darwin" ]]; then
    if command -v terminal-notifier &>/dev/null; then
        terminal-notifier \
            -title "Claude Code" \
            -subtitle "$LOCATION" \
            -message "$MESSAGE" \
            -execute "tlink open '$DEEPLINK'" &
    else
        osascript -e "display notification \"$MESSAGE\" with title \"Claude Code\" subtitle \"$LOCATION\" sound name \"Glass\""
    fi
else
    if command -v dunstify &>/dev/null; then
        (
            ACTION=$(dunstify "Claude Code — $LOCATION" "$MESSAGE" \
                --action="default,Go there" \
                --urgency=normal \
                --icon=utilities-terminal \
                --appname="Claude Code")
            [ "$ACTION" = "default" ] && tlink open "$DEEPLINK"
        ) &
    elif command -v notify-send &>/dev/null; then
        notify-send "Claude Code — $LOCATION" "$MESSAGE" \
            --urgency=normal \
            --icon=utilities-terminal \
            --app-name="Claude Code"
    fi
fi
"##;

pub fn hook_script_path() -> PathBuf {
    dirs::home_dir()
        .expect("home dir not found")
        .join(".config/tlink/hooks/claude-notification.sh")
}

fn claude_settings_path() -> PathBuf {
    dirs::home_dir()
        .expect("home dir not found")
        .join(".claude/settings.json")
}

pub fn is_installed() -> bool {
    hook_script_path().exists()
}

pub fn install() -> Result<()> {
    let script = hook_script_path();
    if let Some(p) = script.parent() {
        std::fs::create_dir_all(p)?;
    }
    std::fs::write(&script, HOOK_SCRIPT)?;
    std::process::Command::new("chmod")
        .args(["+x", script.to_str().unwrap()])
        .status()?;

    register_hook(script.to_str().unwrap())?;

    println!("claude-notification installed.");
    println!("  Hook:     {}", script.display());
    println!("  Settings: {}", claude_settings_path().display());
    println!();
    println!("For click-to-navigate support:");
    println!("  macOS: brew install terminal-notifier");
    println!("  Linux: apt install dunst  /  pacman -S dunst");
    Ok(())
}

pub fn uninstall() -> Result<()> {
    let script = hook_script_path();
    if script.exists() {
        std::fs::remove_file(&script)?;
    }
    deregister_hook()?;
    println!("claude-notification removed.");
    Ok(())
}

fn register_hook(script_path: &str) -> Result<()> {
    let path = claude_settings_path();
    let content = if path.exists() {
        std::fs::read_to_string(&path)?
    } else {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p)?;
        }
        "{}".to_string()
    };

    let mut settings: serde_json::Value =
        serde_json::from_str(&content).context("~/.claude/settings.json is not valid JSON")?;

    // Ensure hooks.Notification exists as an array
    if settings["hooks"]["Notification"].as_array().is_none() {
        settings["hooks"]["Notification"] = serde_json::json!([]);
    }

    // Remove any existing tlink entry to avoid duplicates
    let arr = settings["hooks"]["Notification"].as_array_mut().unwrap();
    arr.retain(|e| {
        !e["hooks"][0]["command"]
            .as_str()
            .unwrap_or("")
            .contains("claude-notification")
    });
    arr.push(serde_json::json!({
        "matcher": "",
        "hooks": [{ "type": "command", "command": script_path }]
    }));

    std::fs::write(&path, serde_json::to_string_pretty(&settings)?)?;
    Ok(())
}

fn deregister_hook() -> Result<()> {
    let path = claude_settings_path();
    if !path.exists() {
        return Ok(());
    }
    let content = std::fs::read_to_string(&path)?;
    let mut settings: serde_json::Value =
        serde_json::from_str(&content).context("~/.claude/settings.json is not valid JSON")?;

    if let Some(arr) = settings["hooks"]["Notification"].as_array_mut() {
        arr.retain(|e| {
            !e["hooks"][0]["command"]
                .as_str()
                .unwrap_or("")
                .contains("claude-notification")
        });
    }
    std::fs::write(&path, serde_json::to_string_pretty(&settings)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_script_path_in_config_dir() {
        let p = hook_script_path();
        assert!(p.to_string_lossy().contains(".config/tlink/hooks"));
        assert!(p.to_string_lossy().ends_with("claude-notification.sh"));
    }

    #[test]
    fn test_hook_captures_tmux_context() {
        assert!(HOOK_SCRIPT.contains("tmux display-message"));
        assert!(HOOK_SCRIPT.contains("session_name"));
        assert!(HOOK_SCRIPT.contains("window_name"));
        assert!(HOOK_SCRIPT.contains("pane_index"));
    }

    #[test]
    fn test_hook_does_not_expose_deeplink_in_notification_text() {
        // The tmux:// URL is only passed to -execute / tlink open,
        // never shown as the visible notification title/body.
        assert!(HOOK_SCRIPT.contains("-subtitle \"$LOCATION\""));
        assert!(!HOOK_SCRIPT.contains("-subtitle \"$DEEPLINK\""));
        assert!(!HOOK_SCRIPT.contains("-message \"$DEEPLINK\""));
    }

    #[test]
    fn test_hook_supports_click_navigate_macos() {
        assert!(HOOK_SCRIPT.contains("terminal-notifier"));
        assert!(HOOK_SCRIPT.contains("-execute"));
        assert!(HOOK_SCRIPT.contains("tlink open"));
    }

    #[test]
    fn test_hook_supports_click_navigate_linux() {
        assert!(HOOK_SCRIPT.contains("dunstify"));
        assert!(HOOK_SCRIPT.contains("tlink open \"$DEEPLINK\""));
    }
}
