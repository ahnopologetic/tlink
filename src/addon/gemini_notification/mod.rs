mod wizard;

use anyhow::{Context, Result};
use std::path::PathBuf;

// ── Notification methods (reuse from claude_notification) ─────────────────────

#[derive(Clone, PartialEq, Debug)]
pub enum NotifMethod {
    TerminalNotifier,
    Osascript,
    Dunstify,
    NotifySend,
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

    pub fn platform_methods() -> Vec<Self> {
        if cfg!(target_os = "macos") {
            vec![Self::TerminalNotifier, Self::Osascript]
        } else {
            vec![Self::Dunstify, Self::NotifySend]
        }
    }

    pub fn recommended_method() -> Self {
        if cfg!(target_os = "macos") {
            Self::TerminalNotifier
        } else {
            Self::Dunstify
        }
    }
}

/// Gemini CLI hook events that can trigger desktop notifications.
#[derive(Clone, PartialEq, Debug)]
pub enum HookEvent {
    AfterAgent,    // Agent finished responding
    SessionStart,  // Session started
    SessionEnd,    // Session ended
    TaskCreated,   // A task was created
    TaskCompleted, // A task was completed
    BeforeTool,    // Before a tool runs
    AfterTool,     // After a tool runs
}

impl HookEvent {
    pub fn event_key(&self) -> &'static str {
        match self {
            Self::AfterAgent => "AfterAgent",
            Self::SessionStart => "SessionStart",
            Self::SessionEnd => "SessionEnd",
            Self::TaskCreated => "TaskCreated",
            Self::TaskCompleted => "TaskCompleted",
            Self::BeforeTool => "BeforeTool",
            Self::AfterTool => "AfterTool",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::AfterAgent => "After Agent",
            Self::SessionStart => "Session Start",
            Self::SessionEnd => "Session End",
            Self::TaskCreated => "Task Created",
            Self::TaskCompleted => "Task Completed",
            Self::BeforeTool => "Before Tool",
            Self::AfterTool => "After Tool",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::AfterAgent => "Gemini CLI finished responding and is waiting for input",
            Self::SessionStart => "A Gemini CLI session has started",
            Self::SessionEnd => "A Gemini CLI session has ended",
            Self::TaskCreated => "A new task has been created",
            Self::TaskCompleted => "A task has been completed",
            Self::BeforeTool => "Before a tool execution begins",
            Self::AfterTool => "After a tool execution completes",
        }
    }

    pub fn category(&self) -> &'static str {
        match self {
            Self::AfterAgent => "Agent",
            Self::SessionStart | Self::SessionEnd => "Session",
            Self::TaskCreated | Self::TaskCompleted => "Tasks",
            Self::BeforeTool | Self::AfterTool => "Tools",
        }
    }
}

pub const GEMINI_CATEGORIES: &[&str] = &["Agent", "Session", "Tasks", "Tools"];

pub struct InstallOptions {
    pub method: NotifMethod,
    pub events: Vec<HookEvent>,
}

// ── Paths ─────────────────────────────────────────────────────────────────────

pub fn hook_script_path() -> PathBuf {
    dirs::home_dir()
        .expect("home dir not found")
        .join(".config/tlink/hooks/gemini-notification.sh")
}

fn gemini_settings_path() -> PathBuf {
    dirs::home_dir()
        .expect("home dir not found")
        .join(".gemini/settings.json")
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
            println!("✓ gemini-notification installed.");
            println!("  Hook:     {}", hook_script_path().display());
            println!("  Settings: {}", gemini_settings_path().display());
            println!("  Restart Gemini CLI to activate.");
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
    println!("gemini-notification removed.");
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

    let mut config = crate::config::load().unwrap_or_default();
    config.notification_method = Some(opts.method.config_key().to_string());
    crate::config::save(&config)?;

    // Group selected events by their event_key and register each group
    for (event_key, matcher) in build_registrations(&opts.events) {
        register_hook_entry(&event_key, script.to_str().unwrap(), &matcher)?;
    }

    Ok(())
}

/// Groups events by event_key.
/// For Gemini hooks, the matcher is always empty (match all) since Gemini uses
/// `"*"` wildcard matcher semantics.
fn build_registrations(events: &[HookEvent]) -> Vec<(String, String)> {
    use std::collections::BTreeMap;

    let mut map: BTreeMap<&str, bool> = BTreeMap::new();

    for event in events {
        map.entry(event.event_key()).or_insert(true);
    }

    map.into_keys()
        .map(|k| (k.to_string(), "*".to_string()))
        .collect()
}

fn generate_hook_script(method: &NotifMethod) -> String {
    // DEEPLINK is only passed to the click action, never shown as visible text
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
            --appname="Gemini CLI")
        [ "$ACTION" = "default" ] && tlink open "$DEEPLINK"
    ) &"##
        }
        NotifMethod::NotifySend => {
            r##"    notify-send "$NOTIF_TITLE" "$MESSAGE" \
        --urgency=normal \
        --icon=utilities-terminal \
        --app-name="Gemini CLI" \
        --hint=string:body:"$LOCATION""##
        }
    };

    format!(
        r##"#!/bin/bash
# tlink gemini-notification hook
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
}}
try:
    d    = json.loads(sys.stdin.read())
    msg  = d.get('message', '').replace(chr(10), ' ')
    kind = d.get('notification_type', '')
    if msg:
        print('MESSAGE='     + shlex.quote(msg))
    else:
        print('MESSAGE='     + shlex.quote('Gemini CLI notification'))
    print('NOTIF_TITLE=' + shlex.quote(TYPES.get(kind, 'Gemini CLI')))
except Exception:
    print(\"MESSAGE='Gemini CLI notification'\")
    print(\"NOTIF_TITLE='Gemini CLI'\")
" 2>/dev/null || echo "MESSAGE='Gemini CLI notification'; NOTIF_TITLE='Gemini CLI'")"

DEEPLINK="tmux://${{SESSION}}/${{WINDOW}}/${{PANE}}"
LOCATION="${{SESSION}} > ${{WINDOW}} > ${{PANE}}"

{notify_block}
"##,
        method_label = method.label(),
        notify_block = notify_block,
    )
}

fn register_hook_entry(event_key: &str, script_path: &str, matcher: &str) -> Result<()> {
    let path = gemini_settings_path();
    let content = if path.exists() {
        std::fs::read_to_string(&path)?
    } else {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p)?;
        }
        "{}".to_string()
    };

    let mut settings: serde_json::Value =
        serde_json::from_str(&content).context("~/.gemini/settings.json is not valid JSON")?;

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
    let path = gemini_settings_path();
    if !path.exists() {
        return Ok(());
    }
    let content = std::fs::read_to_string(&path)?;
    let mut settings: serde_json::Value =
        serde_json::from_str(&content).context("~/.gemini/settings.json is not valid JSON")?;

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
    e["command"]
        .as_str()
        .unwrap_or("")
        .contains("gemini-notification")
        || e["hooks"][0]["command"]
            .as_str()
            .unwrap_or("")
            .contains("gemini-notification")
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_script_path_in_config_dir() {
        let p = hook_script_path();
        assert!(p.to_string_lossy().contains(".config/tlink/hooks"));
        assert!(p.to_string_lossy().ends_with("gemini-notification.sh"));
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
    fn test_generate_script_does_not_expose_deeplink_in_text() {
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
    fn test_build_registrations_after_agent() {
        let r = build_registrations(&[HookEvent::AfterAgent]);
        assert_eq!(r, vec![("AfterAgent".to_string(), "*".to_string())]);
    }

    #[test]
    fn test_build_registrations_multiple_events() {
        let r = build_registrations(&[HookEvent::AfterAgent, HookEvent::SessionStart]);
        assert_eq!(r.len(), 2);
        let keys: Vec<&str> = r.iter().map(|(k, _)| k.as_str()).collect();
        assert!(keys.contains(&"AfterAgent"));
        assert!(keys.contains(&"SessionStart"));
    }

    #[test]
    fn test_all_events_have_keys() {
        for e in &[
            HookEvent::AfterAgent,
            HookEvent::SessionStart,
            HookEvent::SessionEnd,
            HookEvent::TaskCreated,
            HookEvent::TaskCompleted,
            HookEvent::BeforeTool,
            HookEvent::AfterTool,
        ] {
            assert!(!e.event_key().is_empty());
            assert!(!e.label().is_empty());
            assert!(!e.description().is_empty());
        }
    }
}
