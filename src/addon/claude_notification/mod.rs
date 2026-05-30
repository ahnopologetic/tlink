mod wizard;

use anyhow::{Context, Result};
use std::path::PathBuf;

// ── Option types (shared with wizard) ────────────────────────────────────────

#[derive(Clone, PartialEq, Debug)]
pub enum NotifMethod {
    TerminalNotifier, // macOS — banner notifications
    Osascript,        // macOS — built-in fallback, no click callback
    Dunstify,         // Linux — click-to-navigate via action
    NotifySend,       // Linux — basic
}

impl NotifMethod {
    pub fn label(&self) -> &'static str {
        match self {
            Self::TerminalNotifier => "terminal-notifier",
            Self::Osascript => "osascript (built-in)",
            Self::Dunstify => "dunstify",
            Self::NotifySend => "notify-send",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::TerminalNotifier => {
                "macOS banner notifications. Install: brew install terminal-notifier"
            }
            Self::Osascript => {
                "Built-in macOS. No click callback — navigates immediately when notification fires"
            }
            Self::Dunstify => "Click notification to jump back — requires: dunst daemon",
            Self::NotifySend => "Desktop notification, no click action — requires: libnotify",
        }
    }

    pub fn available(&self) -> bool {
        let cmd = match self {
            Self::TerminalNotifier => "terminal-notifier",
            Self::Osascript => "osascript",
            Self::Dunstify => "dunstify",
            Self::NotifySend => "notify-send",
        };
        std::process::Command::new("which")
            .arg(cmd)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    pub fn config_key(&self) -> &'static str {
        match self {
            Self::TerminalNotifier => "terminal-notifier",
            Self::Osascript => "osascript",
            Self::Dunstify => "dunstify",
            Self::NotifySend => "notify-send",
        }
    }

    #[allow(dead_code)]
    pub fn from_config_key(key: &str) -> Option<Self> {
        match key {
            "terminal-notifier" => Some(Self::TerminalNotifier),
            "osascript" => Some(Self::Osascript),
            "dunstify" => Some(Self::Dunstify),
            "notify-send" => Some(Self::NotifySend),
            _ => None,
        }
    }

    /// Ordered list of methods for the current platform, best-first.
    pub fn platform_methods() -> Vec<Self> {
        if cfg!(target_os = "macos") {
            vec![Self::TerminalNotifier, Self::Osascript]
        } else {
            vec![Self::Dunstify, Self::NotifySend]
        }
    }

    /// The single best method for this platform and OS version.
    pub fn recommended_method() -> Self {
        if cfg!(target_os = "macos") {
            Self::TerminalNotifier
        } else {
            Self::Dunstify
        }
    }
}

/// All Claude Code hook events worth surfacing as desktop notifications.
/// `event_key()` is the top-level key in settings.json hooks.
/// `matcher()` is the filter within that key (empty = match all).
#[derive(Clone, PartialEq, Debug)]
pub enum HookEvent {
    // ── Notification sub-types (key: "Notification") ──────────────────────
    NotificationIdle,           // idle_prompt
    NotificationPermission,     // permission_prompt
    NotificationAuth,           // auth_success
    NotificationElicitDialog,   // elicitation_dialog
    NotificationElicitComplete, // elicitation_complete
    NotificationElicitResponse, // elicitation_response
    AllNotifications,           // all Notification sub-types

    // ── Turn lifecycle ────────────────────────────────────────────────────
    Stop,        // Claude finished responding
    StopFailure, // API error ended the turn

    // ── Tool execution ────────────────────────────────────────────────────
    PostToolUse,
    PostToolUseFailure,

    // ── Agents & Tasks ────────────────────────────────────────────────────
    SubagentStop,
    TeammateIdle,
    TaskCreated,
    TaskCompleted,

    // ── Session ───────────────────────────────────────────────────────────
    SessionStart,
    SessionEnd,
}

impl HookEvent {
    /// Top-level key in settings.json hooks object.
    pub fn event_key(&self) -> &'static str {
        match self {
            Self::NotificationIdle
            | Self::NotificationPermission
            | Self::NotificationAuth
            | Self::NotificationElicitDialog
            | Self::NotificationElicitComplete
            | Self::NotificationElicitResponse
            | Self::AllNotifications => "Notification",
            Self::Stop => "Stop",
            Self::StopFailure => "StopFailure",
            Self::PostToolUse => "PostToolUse",
            Self::PostToolUseFailure => "PostToolUseFailure",
            Self::SubagentStop => "SubagentStop",
            Self::TeammateIdle => "TeammateIdle",
            Self::TaskCreated => "TaskCreated",
            Self::TaskCompleted => "TaskCompleted",
            Self::SessionStart => "SessionStart",
            Self::SessionEnd => "SessionEnd",
        }
    }

    /// Matcher within the event key. Empty = match all instances.
    pub fn matcher(&self) -> &'static str {
        match self {
            Self::NotificationIdle => "idle_prompt",
            Self::NotificationPermission => "permission_prompt",
            Self::NotificationAuth => "auth_success",
            Self::NotificationElicitDialog => "elicitation_dialog",
            Self::NotificationElicitComplete => "elicitation_complete",
            Self::NotificationElicitResponse => "elicitation_response",
            _ => "", // match all for this event key
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::NotificationIdle => "idle_prompt",
            Self::NotificationPermission => "permission_prompt",
            Self::NotificationAuth => "auth_success",
            Self::NotificationElicitDialog => "elicitation_dialog",
            Self::NotificationElicitComplete => "elicitation_complete",
            Self::NotificationElicitResponse => "elicitation_response",
            Self::AllNotifications => "all notifications",
            Self::Stop => "Stop",
            Self::StopFailure => "StopFailure",
            Self::PostToolUse => "PostToolUse",
            Self::PostToolUseFailure => "PostToolUseFailure",
            Self::SubagentStop => "SubagentStop",
            Self::TeammateIdle => "TeammateIdle",
            Self::TaskCreated => "TaskCreated",
            Self::TaskCompleted => "TaskCompleted",
            Self::SessionStart => "SessionStart",
            Self::SessionEnd => "SessionEnd",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::NotificationIdle => "Claude finished a task and is waiting for you",
            Self::NotificationPermission => "Claude needs your approval before running a tool",
            Self::NotificationAuth => "Authentication token was refreshed",
            Self::NotificationElicitDialog => "An MCP server is asking you a question via Claude",
            Self::NotificationElicitComplete => "An MCP elicitation dialog finished",
            Self::NotificationElicitResponse => "Your response was submitted to an MCP server",
            Self::AllNotifications => "All 6 Notification sub-types",
            Self::Stop => "Claude finished responding (good for long tasks)",
            Self::StopFailure => "Turn ended due to an API error",
            Self::PostToolUse => "A tool call completed successfully",
            Self::PostToolUseFailure => "A tool call failed",
            Self::SubagentStop => "A subagent finished its work",
            Self::TeammateIdle => "A teammate agent is waiting for your input",
            Self::TaskCreated => "A new task was created via TaskCreate",
            Self::TaskCompleted => "A task was marked as completed",
            Self::SessionStart => "A Claude Code session started or resumed",
            Self::SessionEnd => "A Claude Code session ended",
        }
    }

    pub fn category(&self) -> &'static str {
        match self {
            Self::NotificationIdle
            | Self::NotificationPermission
            | Self::NotificationAuth
            | Self::NotificationElicitDialog
            | Self::NotificationElicitComplete
            | Self::NotificationElicitResponse
            | Self::AllNotifications => "Notifications",
            Self::Stop | Self::StopFailure => "Turn",
            Self::PostToolUse | Self::PostToolUseFailure => "Tools",
            Self::SubagentStop | Self::TeammateIdle | Self::TaskCreated | Self::TaskCompleted => {
                "Agents & Tasks"
            }
            Self::SessionStart | Self::SessionEnd => "Session",
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
            if !opts.method.available() && opts.method == NotifMethod::TerminalNotifier {
                println!("Installing terminal-notifier via Homebrew…");
                let ok = std::process::Command::new("brew")
                    .args(["install", "terminal-notifier"])
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false);
                if !ok {
                    eprintln!("warning: brew install terminal-notifier failed.");
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

    std::fs::write(&script, hook_script())?;
    std::process::Command::new("chmod")
        .args(["+x", script.to_str().unwrap()])
        .status()?;

    let mut config = crate::config::load().unwrap_or_default();
    config.notification_method = Some(opts.method.config_key().to_string());
    crate::config::save(&config)?;

    // Group selected events by their settings.json key and register each group.
    for (event_key, matcher) in build_registrations(&opts.events) {
        register_hook_entry(&event_key, script.to_str().unwrap(), &matcher)?;
    }

    Ok(())
}

/// Groups events by event_key. Within each key, Notification sub-type matchers
/// are pipe-joined; an empty matcher (match-all) overrides specific ones.
fn build_registrations(events: &[HookEvent]) -> Vec<(String, String)> {
    use std::collections::BTreeMap;

    // BTreeMap<event_key, Option<Vec<matcher>>>
    // None  = match-all (empty matcher string)
    // Some  = list of specific matchers to pipe-join
    let mut map: BTreeMap<&str, Option<Vec<&str>>> = BTreeMap::new();

    for event in events {
        let key = event.event_key();
        let matcher = event.matcher();

        match map.entry(key) {
            std::collections::btree_map::Entry::Vacant(e) => {
                if matcher.is_empty() {
                    e.insert(None);
                } else {
                    e.insert(Some(vec![matcher]));
                }
            }
            std::collections::btree_map::Entry::Occupied(mut e) => {
                if matcher.is_empty() {
                    *e.get_mut() = None; // upgrade to match-all
                } else if let Some(ref mut v) = e.get_mut() {
                    if !v.contains(&matcher) {
                        v.push(matcher);
                    }
                }
                // already None (match-all) → keep as is
            }
        }
    }

    map.into_iter()
        .map(|(k, v)| (k.to_string(), v.map_or(String::new(), |ms| ms.join("|"))))
        .collect()
}

/// Thin bash wrapper — captures tmux context, delegates JSON parsing and
/// notification firing entirely to `tlink notify` (Rust).
pub fn hook_script() -> &'static str {
    r##"#!/bin/bash
SESSION=$(tmux display-message -p "#{session_name}" 2>/dev/null) || exit 0
WINDOW=$(tmux display-message -p "#{window_name}" 2>/dev/null) || exit 0
PANE=$(tmux display-message -p "#{pane_index}" 2>/dev/null) || exit 0
[ -z "$SESSION" ] && exit 0
exec tlink notify --session "$SESSION" --window "$WINDOW" --pane "$PANE"
"##
}

#[allow(dead_code)]
pub fn generate_hook_script(method: &NotifMethod) -> String {
    // DEEPLINK is only passed to the click action (-execute / tlink open),
    // never shown as visible notification text.
    let notify_block = match method {
        NotifMethod::TerminalNotifier => {
            r##"    terminal-notifier \
        -title "$NOTIF_TITLE" \
        -subtitle "$LOCATION" \
        -message "$MESSAGE" \
        -execute "tlink open $DEEPLINK" &"##
        }

        NotifMethod::Osascript => {
            r##"    osascript -e "display notification \"$MESSAGE\" with title \"$NOTIF_TITLE\" subtitle \"$LOCATION\" sound name \"Glass\""
"##
        }

        NotifMethod::Dunstify => {
            r##"    (
        ACTION=$(dunstify "$NOTIF_TITLE" "$MESSAGE" \
            --hint=string:x-dunst-stack-tag:tlink \
            --action="default,Go there" \
            --urgency=normal \
            --icon=utilities-terminal \
            --appname="Claude Code")
        [ "$ACTION" = "default" ] && tlink open "$DEEPLINK"
    ) &"##
        }

        NotifMethod::NotifySend => {
            r##"    notify-send "$NOTIF_TITLE" "$MESSAGE" \
        --urgency=normal \
        --icon=utilities-terminal \
        --app-name="Claude Code" \
        --hint=string:body:"$LOCATION""##
        }
    };

    // Parse message + notification_type in one Python call; use eval to set
    // both MESSAGE and NOTIF_TITLE safely (shlex.quote handles escaping).
    format!(
        r##"#!/bin/bash
# tlink claude-notification hook
# method: {method_label}

SESSION=$(tmux display-message -p "#{{session_name}}" 2>/dev/null) || exit 0
WINDOW=$(tmux display-message -p "#{{window_name}}" 2>/dev/null) || exit 0
PANE=$(tmux display-message -p "#{{pane_index}}" 2>/dev/null) || exit 0
[ -z "$SESSION" ] && exit 0

INPUT=$(cat)
eval "$(printf '%s' "$INPUT" | python3 -c "
import sys, json, shlex
TYPES = {{
    'idle_prompt':          'Waiting for your input',
    'permission_prompt':    'Permission needed',
    'auth_success':         'Authenticated',
    'elicitation_dialog':   'MCP: question for you',
    'elicitation_complete': 'MCP: dialog complete',
    'elicitation_response': 'MCP: response submitted',
}}
try:
    d    = json.loads(sys.stdin.read())
    msg  = d.get('message', 'Claude notification').replace(chr(10), ' ')
    kind = d.get('notification_type', '')
    print('MESSAGE='     + shlex.quote(msg))
    print('NOTIF_TITLE=' + shlex.quote(TYPES.get(kind, 'Claude Code')))
except Exception:
    print(\"MESSAGE='Claude notification'\")
    print(\"NOTIF_TITLE='Claude Code'\")
" 2>/dev/null || echo "MESSAGE='Claude notification'; NOTIF_TITLE='Claude Code'")"

DEEPLINK="tmux://${{SESSION}}/${{WINDOW}}/${{PANE}}"
LOCATION="${{SESSION}} > ${{WINDOW}} > ${{PANE}}"

{notify_block}
"##,
        method_label = method.label(),
        notify_block = notify_block,
    )
}

fn register_hook_entry(event_key: &str, script_path: &str, matcher: &str) -> Result<()> {
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

    if !settings["hooks"].is_object() {
        settings["hooks"] = serde_json::json!({});
    }

    {
        let hooks_obj = settings["hooks"].as_object_mut().unwrap();
        let arr = hooks_obj
            .entry(event_key)
            .or_insert_with(|| serde_json::json!([]));
        if let Some(v) = arr.as_array_mut() {
            v.retain(|e| !is_tlink_hook(e));
            v.push(serde_json::json!({
                "matcher": matcher,
                "hooks": [{ "type": "command", "command": script_path }]
            }));
        }
    }

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

    // Remove tlink entries from ALL event keys, not just Notification.
    if let Some(hooks) = settings["hooks"].as_object_mut() {
        for arr_val in hooks.values_mut() {
            if let Some(arr) = arr_val.as_array_mut() {
                arr.retain(|e| !is_tlink_hook(e));
            }
        }
    }

    std::fs::write(&path, serde_json::to_string_pretty(&settings)?)?;
    Ok(())
}

fn is_tlink_hook(e: &serde_json::Value) -> bool {
    e["hooks"][0]["command"]
        .as_str()
        .unwrap_or("")
        .contains("claude-notification")
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
        for method in [
            NotifMethod::TerminalNotifier,
            NotifMethod::Osascript,
            NotifMethod::Dunstify,
            NotifMethod::NotifySend,
        ] {
            let s = generate_hook_script(&method);
            assert!(
                s.contains("session_name"),
                "missing session_name for {:?}",
                method
            );
            assert!(
                s.contains("window_name"),
                "missing window_name for {:?}",
                method
            );
            assert!(
                s.contains("pane_index"),
                "missing pane_index for {:?}",
                method
            );
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
    fn test_build_registrations_single_notification() {
        let r = build_registrations(&[HookEvent::NotificationIdle]);
        assert_eq!(
            r,
            vec![("Notification".to_string(), "idle_prompt".to_string())]
        );
    }

    #[test]
    fn test_build_registrations_multiple_notification_sub_types() {
        let r = build_registrations(&[
            HookEvent::NotificationIdle,
            HookEvent::NotificationPermission,
        ]);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].0, "Notification");
        assert!(r[0].1.contains("idle_prompt"));
        assert!(r[0].1.contains("permission_prompt"));
    }

    #[test]
    fn test_build_registrations_all_notifications_overrides() {
        let r = build_registrations(&[HookEvent::NotificationIdle, HookEvent::AllNotifications]);
        assert_eq!(r, vec![("Notification".to_string(), String::new())]);
    }

    #[test]
    fn test_build_registrations_cross_event_keys() {
        let r = build_registrations(&[HookEvent::NotificationIdle, HookEvent::Stop]);
        assert_eq!(r.len(), 2);
        let keys: Vec<&str> = r.iter().map(|(k, _)| k.as_str()).collect();
        assert!(keys.contains(&"Notification"));
        assert!(keys.contains(&"Stop"));
    }

    #[test]
    fn test_build_registrations_stop_has_empty_matcher() {
        let r = build_registrations(&[HookEvent::Stop]);
        assert_eq!(r, vec![("Stop".to_string(), String::new())]);
    }
}
