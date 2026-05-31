mod wizard;

use anyhow::Result;
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

    /// Ordered list of methods for the current platform, best-first.
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

pub struct InstallOptions {
    pub method: NotifMethod,
}

// ── Paths ─────────────────────────────────────────────────────────────────────

pub fn hook_script_path() -> PathBuf {
    dirs::home_dir()
        .expect("home dir not found")
        .join(".config/tlink/hooks/codex-notification.sh")
}

fn codex_config_path() -> PathBuf {
    dirs::home_dir()
        .expect("home dir not found")
        .join(".codex/config.toml")
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
            println!("✓ codex-notification installed.");
            println!("  Hook:     {}", hook_script_path().display());
            println!("  Config:   {}", codex_config_path().display());
            println!("  Restart Codex CLI to activate.");
            Ok(())
        }
    }
}

pub fn uninstall() -> Result<()> {
    let script = hook_script_path();
    if script.exists() {
        std::fs::remove_file(&script)?;
    }

    // Remove notify config from codex config.toml
    remove_notify_config()?;
    println!("codex-notification removed.");
    Ok(())
}

// ── Installation logic ────────────────────────────────────────────────────────

/// Thin bash wrapper — captures tmux context and the "turn-ended" argument,
/// pipes a JSON payload to `tlink notify` (Rust), which fires the desktop
/// notification via the configured backend.
/// The `--source codex` flag tells tlink which agent adapter to use.
pub fn hook_script() -> &'static str {
    r##"#!/bin/bash
SESSION=$(tmux display-message -p "#{session_name}" 2>/dev/null) || exit 0
WINDOW=$(tmux display-message -p "#{window_name}" 2>/dev/null) || exit 0
PANE=$(tmux display-message -p "#{pane_index}" 2>/dev/null) || exit 0
[ -z "$SESSION" ] && exit 0
STATUS="${1:-turn-ended}"
printf '{"source":"codex","status":"%s"}\n' "$STATUS" | tlink notify --source codex --session "$SESSION" --window "$WINDOW" --pane "$PANE"
"##
}

pub fn install_with_options(opts: &InstallOptions) -> Result<()> {
    let script = hook_script_path();
    if let Some(p) = script.parent() {
        std::fs::create_dir_all(p)?;
    }

    std::fs::write(&script, hook_script())?;
    std::process::Command::new("chmod")
        .args(["+x", script.to_str().unwrap()])
        .status()?;

    // Save notification method
    let mut config = crate::config::load().unwrap_or_default();
    config.notification_method = Some(opts.method.config_key().to_string());
    crate::config::save(&config)?;

    // Register notify in codex config.toml
    register_notify_config(script.to_str().unwrap())?;

    Ok(())
}

/// Generate a bash hook script for Codex CLI (legacy — see hook_script())
#[allow(dead_code)]
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
            --appname="Codex CLI")
        [ "$ACTION" = "default" ] && tlink open "$DEEPLINK"
    ) &"##
        }
        NotifMethod::NotifySend => {
            r##"    notify-send "$NOTIF_TITLE" "$MESSAGE" \
        --urgency=normal \
        --icon=utilities-terminal \
        --app-name="Codex CLI" \
        --hint=string:body:"$LOCATION""##
        }
    };

    format!(
        r##"#!/bin/bash
# tlink codex-notification hook
# method: {method_label}

SESSION=$(tmux display-message -p "#{{session_name}}" 2>/dev/null) || exit 0
WINDOW=$(tmux display-message -p "#{{window_name}}" 2>/dev/null) || exit 0
PANE=$(tmux display-message -p "#{{pane_index}}" 2>/dev/null) || exit 0
[ -z "$SESSION" ] && exit 0
TERMTYPE=$(tmux display-message -p "#{{client_termtype}}" 2>/dev/null || echo "")

MESSAGE="Codex CLI task completed"
NOTIF_TITLE="Codex CLI"
TERM_NAME="${{TERMTYPE%% *}}"
DEEPLINK="tmux://${{SESSION}}/${{WINDOW}}/${{PANE}}"
[ -n "$TERM_NAME" ] && DEEPLINK="${{DEEPLINK}}?term=${{TERM_NAME}}"
LOCATION="${{SESSION}} > ${{WINDOW}} > ${{PANE}}"

{notify_block}
"##,
        method_label = method.label(),
        notify_block = notify_block,
    )
}

fn register_notify_config(script_path: &str) -> Result<()> {
    let path = codex_config_path();
    let content = if path.exists() {
        std::fs::read_to_string(&path)?
    } else {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p)?;
        }
        String::new()
    };

    let mut config: toml::Value = content
        .parse()
        .unwrap_or(toml::Value::Table(toml::map::Map::new()));

    // Set notify = ["/path/to/script", "turn-ended"]
    if let toml::Value::Table(ref mut table) = config {
        table.insert(
            "notify".to_string(),
            toml::Value::Array(vec![
                toml::Value::String(script_path.to_string()),
                toml::Value::String("turn-ended".to_string()),
            ]),
        );
    }

    std::fs::write(&path, toml::to_string(&config)?)?;
    Ok(())
}

fn remove_notify_config() -> Result<()> {
    let path = codex_config_path();
    if !path.exists() {
        return Ok(());
    }

    let content = std::fs::read_to_string(&path)?;
    let mut config: toml::Value = content
        .parse()
        .unwrap_or(toml::Value::Table(toml::map::Map::new()));

    // Remove the notify key
    if let toml::Value::Table(ref mut table) = config {
        table.remove("notify");
    }

    std::fs::write(&path, toml::to_string(&config)?)?;
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
        assert!(p.to_string_lossy().ends_with("codex-notification.sh"));
    }

    #[test]
    fn test_hook_script_captures_tmux_context() {
        let s = hook_script();
        assert!(s.contains("session_name"), "missing session_name");
        assert!(s.contains("window_name"), "missing window_name");
        assert!(s.contains("pane_index"), "missing pane_index");
    }

    #[test]
    fn test_hook_script_has_source_codex() {
        let s = hook_script();
        assert!(s.contains("--source codex"), "missing --source codex flag");
    }

    #[test]
    fn test_hook_script_calls_tlink_notify() {
        let s = hook_script();
        assert!(s.contains("tlink notify"), "missing tlink notify call");
    }

    #[test]
    fn test_hook_script_sends_json_status() {
        let s = hook_script();
        assert!(s.contains("\"source\":\"codex\""), "missing source in JSON");
        assert!(s.contains("\"status\""), "missing status in JSON");
    }

    #[test]
    fn test_hook_script_uses_turn_ended_default() {
        let s = hook_script();
        assert!(s.contains("turn-ended"), "missing turn-ended default");
    }
}
