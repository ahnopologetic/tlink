mod wizard;

use anyhow::{Context, Result};
use std::path::PathBuf;

// ── Option types (shared with wizard) ────────────────────────────────────────

#[derive(Clone, PartialEq, Debug)]
pub enum NotifMethod {
    TerminalNotifier, // macOS — click-to-navigate via -execute
    Osascript,        // macOS — basic, no click action
    Dunstify,         // Linux — click-to-navigate via action
    NotifySend,       // Linux — basic
}

impl NotifMethod {
    pub fn label(&self) -> &'static str {
        match self {
            Self::TerminalNotifier => "terminal-notifier",
            Self::Osascript        => "osascript (built-in)",
            Self::Dunstify         => "dunstify",
            Self::NotifySend       => "notify-send",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::TerminalNotifier => "Click notification to jump back — requires: brew install terminal-notifier",
            Self::Osascript        => "System alert, no click action — always available on macOS",
            Self::Dunstify         => "Click notification to jump back — requires: dunst daemon",
            Self::NotifySend       => "Desktop notification, no click action — requires: libnotify",
        }
    }

    pub fn available(&self) -> bool {
        let cmd = match self {
            Self::TerminalNotifier => "terminal-notifier",
            Self::Osascript        => "osascript",
            Self::Dunstify         => "dunstify",
            Self::NotifySend       => "notify-send",
        };
        std::process::Command::new("which")
            .arg(cmd)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    pub fn platform_methods() -> Vec<Self> {
        if cfg!(target_os = "macos") {
            vec![Self::TerminalNotifier, Self::Osascript]
        } else {
            vec![Self::Dunstify, Self::NotifySend]
        }
    }
}

/// All notification_type values Claude Code can emit (as of current docs).
/// Matcher is pipe-separated exact values; empty string matches all.
#[derive(Clone, PartialEq, Debug)]
pub enum HookEvent {
    // ── High-signal (default on) ───────────────────────────────────────────
    IdlePrompt,        // Claude finished and is waiting — most useful
    PermissionPrompt,  // Claude needs your approval to proceed
    // ── Lower-signal (opt-in) ─────────────────────────────────────────────
    AuthSuccess,             // Authentication token refreshed
    ElicitationDialog,       // MCP server is asking you a question via Claude
    ElicitationComplete,     // MCP dialog interaction finished
    ElicitationResponse,     // Your response was submitted to the MCP server
    // ── Catch-all ─────────────────────────────────────────────────────────
    All,
}

impl HookEvent {
    pub fn label(&self) -> &'static str {
        match self {
            Self::IdlePrompt          => "idle_prompt",
            Self::PermissionPrompt    => "permission_prompt",
            Self::AuthSuccess         => "auth_success",
            Self::ElicitationDialog   => "elicitation_dialog",
            Self::ElicitationComplete => "elicitation_complete",
            Self::ElicitationResponse => "elicitation_response",
            Self::All                 => "all events",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::IdlePrompt          => "Claude finished a task and is waiting for your input",
            Self::PermissionPrompt    => "Claude needs your approval before running a tool",
            Self::AuthSuccess         => "Authentication token was refreshed",
            Self::ElicitationDialog   => "An MCP server is asking you a question via Claude",
            Self::ElicitationComplete => "An MCP elicitation dialog finished",
            Self::ElicitationResponse => "Your response was submitted to an MCP server",
            Self::All                 => "Every notification Claude emits (all 6 types)",
        }
    }

    /// Returns the matcher string for settings.json. Pipe-separated for multi-event.
    /// Empty string matches all notification types.
    pub fn matcher(&self) -> &'static str {
        match self {
            Self::IdlePrompt          => "idle_prompt",
            Self::PermissionPrompt    => "permission_prompt",
            Self::AuthSuccess         => "auth_success",
            Self::ElicitationDialog   => "elicitation_dialog",
            Self::ElicitationComplete => "elicitation_complete",
            Self::ElicitationResponse => "elicitation_response",
            Self::All                 => "",
        }
    }
}

pub struct InstallOptions {
    pub method: NotifMethod,
    pub events: Vec<HookEvent>,
}

// ── Paths ─────────────────────────────────────────────────────────────────────

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

// ── Public entry points ───────────────────────────────────────────────────────

pub fn install() -> Result<()> {
    match wizard::run()? {
        None => {
            println!("Installation cancelled.");
            Ok(())
        }
        Some(opts) => {
            // TUI has exited — terminal is back to normal, safe to run brew.
            if opts.method == NotifMethod::TerminalNotifier && !opts.method.available() {
                println!("Installing terminal-notifier via Homebrew…");
                let ok = std::process::Command::new("brew")
                    .args(["install", "terminal-notifier"])
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false);
                if !ok {
                    eprintln!("warning: brew install terminal-notifier failed.");
                    eprintln!("  Notifications will still work via osascript as fallback.");
                }
            }
            install_with_options(&opts)?;
            println!("✓ claude-notification installed.");
            println!("  Hook:     {}", hook_script_path().display());
            println!("  Settings: ~/.claude/settings.json");
            Ok(())
        }
    }
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

// ── Installation logic ────────────────────────────────────────────────────────

pub fn install_with_options(opts: &InstallOptions) -> Result<()> {
    let script = hook_script_path();
    if let Some(p) = script.parent() {
        std::fs::create_dir_all(p)?;
    }

    std::fs::write(&script, generate_hook_script(&opts.method))?;
    std::process::Command::new("chmod")
        .args(["+x", script.to_str().unwrap()])
        .status()?;

    let matcher = build_matcher(&opts.events);
    register_hook(script.to_str().unwrap(), &matcher)?;

    Ok(())
}

fn build_matcher(events: &[HookEvent]) -> String {
    if events.iter().any(|e| *e == HookEvent::All) {
        return String::new();
    }
    events
        .iter()
        .map(|e| e.matcher())
        .collect::<Vec<_>>()
        .join("|")
}

pub fn generate_hook_script(method: &NotifMethod) -> String {
    let notify_block = match method {
        NotifMethod::TerminalNotifier => r##"    terminal-notifier \
        -title "Claude Code" \
        -subtitle "$LOCATION" \
        -message "$MESSAGE" \
        -execute "tlink open '$DEEPLINK'" &"##,

        NotifMethod::Osascript => r##"    osascript -e "display notification \"$MESSAGE\" with title \"Claude Code\" subtitle \"$LOCATION\" sound name \"Glass\""
"##,

        NotifMethod::Dunstify => r##"    (
        ACTION=$(dunstify "Claude Code — $LOCATION" "$MESSAGE" \
            --action="default,Go there" \
            --urgency=normal \
            --icon=utilities-terminal \
            --appname="Claude Code")
        [ "$ACTION" = "default" ] && tlink open "$DEEPLINK"
    ) &"##,

        NotifMethod::NotifySend => r##"    notify-send "Claude Code — $LOCATION" "$MESSAGE" \
        --urgency=normal \
        --icon=utilities-terminal \
        --app-name="Claude Code""##,
    };

    format!(
        r##"#!/bin/bash
# tlink claude-notification hook
# method: {method_label}

SESSION=$(tmux display-message -p "#{{session_name}}" 2>/dev/null) || exit 0
WINDOW=$(tmux display-message -p "#{{window_name}}" 2>/dev/null) || exit 0
PANE=$(tmux display-message -p "#{{pane_index}}" 2>/dev/null) || exit 0
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

DEEPLINK="tmux://${{SESSION}}/${{WINDOW}}/${{PANE}}"
LOCATION="${{SESSION}}:${{WINDOW}}.${{PANE}}"

{notify_block}
"##,
        method_label = method.label(),
        notify_block = notify_block,
    )
}

fn register_hook(script_path: &str, matcher: &str) -> Result<()> {
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

    if settings["hooks"]["Notification"].as_array().is_none() {
        settings["hooks"]["Notification"] = serde_json::json!([]);
    }

    let arr = settings["hooks"]["Notification"].as_array_mut().unwrap();
    arr.retain(|e| {
        !e["hooks"][0]["command"]
            .as_str()
            .unwrap_or("")
            .contains("claude-notification")
    });
    arr.push(serde_json::json!({
        "matcher": matcher,
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

// ── Tests ─────────────────────────────────────────────────────────────────────

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
    fn test_generate_script_captures_tmux_context() {
        for method in [NotifMethod::TerminalNotifier, NotifMethod::Osascript,
                       NotifMethod::Dunstify, NotifMethod::NotifySend] {
            let s = generate_hook_script(&method);
            assert!(s.contains("session_name"), "missing session_name for {:?}", method);
            assert!(s.contains("window_name"),  "missing window_name for {:?}", method);
            assert!(s.contains("pane_index"),   "missing pane_index for {:?}", method);
        }
    }

    #[test]
    fn test_generate_script_does_not_expose_deeplink_in_notification_text() {
        // DEEPLINK is passed to -execute / tlink open, never shown as visible notification text
        let s = generate_hook_script(&NotifMethod::TerminalNotifier);
        assert!(s.contains("-subtitle \"$LOCATION\""));
        assert!(!s.contains("-message \"$DEEPLINK\""));
        assert!(!s.contains("-title \"$DEEPLINK\""));
    }

    #[test]
    fn test_generate_script_terminal_notifier_has_click_action() {
        let s = generate_hook_script(&NotifMethod::TerminalNotifier);
        assert!(s.contains("-execute"));
        assert!(s.contains("tlink open"));
    }

    #[test]
    fn test_generate_script_dunstify_has_click_action() {
        let s = generate_hook_script(&NotifMethod::Dunstify);
        assert!(s.contains("tlink open \"$DEEPLINK\""));
        assert!(s.contains("ACTION"));
    }

    #[test]
    fn test_build_matcher_single_event() {
        let m = build_matcher(&[HookEvent::IdlePrompt]);
        assert_eq!(m, "idle_prompt");
    }

    #[test]
    fn test_build_matcher_multiple_events() {
        let m = build_matcher(&[HookEvent::IdlePrompt, HookEvent::PermissionPrompt]);
        assert_eq!(m, "idle_prompt|permission_prompt");
    }

    #[test]
    fn test_build_matcher_all_events_is_empty() {
        let m = build_matcher(&[HookEvent::All]);
        assert_eq!(m, "");
    }

    #[test]
    fn test_build_matcher_all_overrides_specifics() {
        // If All is in the list, matcher is empty regardless
        let m = build_matcher(&[HookEvent::IdlePrompt, HookEvent::All]);
        assert_eq!(m, "");
    }
}
