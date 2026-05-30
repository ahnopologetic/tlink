mod wizard;

use anyhow::Result;
use std::path::PathBuf;

// ── Pi event types for desktop notifications ──────────────────────────────────

#[derive(Clone, PartialEq, Debug)]
pub enum PiEvent {
    AgentEnd,         // Pi finished responding (equivalent to Stop)
    SessionStart,     // Session started
    SessionShutdown,  // Session ended
    TurnEnd,          // Each LLM turn completed
    ToolExecutionEnd, // Tool completed
}

impl PiEvent {
    pub fn event_key(&self) -> &'static str {
        match self {
            Self::AgentEnd => "agent_end",
            Self::SessionStart => "session_start",
            Self::SessionShutdown => "session_shutdown",
            Self::TurnEnd => "turn_end",
            Self::ToolExecutionEnd => "tool_execution_end",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::AgentEnd => "agent_end",
            Self::SessionStart => "session_start",
            Self::SessionShutdown => "session_shutdown",
            Self::TurnEnd => "turn_end",
            Self::ToolExecutionEnd => "tool_execution_end",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::AgentEnd => "Pi finished responding and is waiting for input",
            Self::SessionStart => "A Pi session has started",
            Self::SessionShutdown => "A Pi session has ended",
            Self::TurnEnd => "An LLM turn completed (good for long tasks)",
            Self::ToolExecutionEnd => "A tool execution completed",
        }
    }

    pub fn category(&self) -> &'static str {
        match self {
            Self::AgentEnd | Self::TurnEnd => "Agent",
            Self::SessionStart | Self::SessionShutdown => "Session",
            Self::ToolExecutionEnd => "Tools",
        }
    }
}

pub const PI_CATEGORIES: &[&str] = &["Agent", "Session", "Tools"];

pub struct InstallOptions {
    pub events: Vec<PiEvent>,
}

// ── Paths ─────────────────────────────────────────────────────────────────────

pub fn extension_path() -> PathBuf {
    dirs::home_dir()
        .expect("home dir not found")
        .join(".pi/agent/extensions/pi-notification.ts")
}

pub fn is_installed() -> bool {
    extension_path().exists()
}

// ── Public entry points ───────────────────────────────────────────────────────

pub fn install() -> Result<()> {
    match wizard::run()? {
        None => {
            println!("Installation cancelled.");
            Ok(())
        }
        Some(opts) => {
            install_with_options(&opts)?;
            println!("✓ pi-notification installed.");
            println!("  Extension: {}", extension_path().display());
            println!("  Reload pi with /reload or restart to activate.");
            Ok(())
        }
    }
}

pub fn uninstall() -> Result<()> {
    let path = extension_path();
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    println!("pi-notification removed.");
    println!("  Reload pi with /reload or restart to deactivate.");
    Ok(())
}

// ── Installation logic ────────────────────────────────────────────────────────

pub fn install_with_options(opts: &InstallOptions) -> Result<()> {
    let path = extension_path();
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }

    let ts = generate_extension(&opts.events);
    std::fs::write(&path, ts)?;
    Ok(())
}

/// Generate the TypeScript extension content for selected pi events.
/// Built piece-by-piece to avoid format! conflicts with JS template `${...}`.
fn generate_extension(events: &[PiEvent]) -> String {
    let has_agent_end = events.contains(&PiEvent::AgentEnd);
    let has_session_start = events.contains(&PiEvent::SessionStart);
    let has_session_shutdown = events.contains(&PiEvent::SessionShutdown);
    let has_turn_end = events.contains(&PiEvent::TurnEnd);
    let has_tool_end = events.contains(&PiEvent::ToolExecutionEnd);

    let mut handlers = String::new();

    if has_agent_end {
        handlers.push_str(
            r#"
pi.on("agent_end", async (event, ctx) => {
  const title = "Pi";
  const body = "Ready for input";
  spawn_notify(title, body, ctx);
});"#,
        );
    }

    if has_session_start {
        handlers.push_str(
            r#"
pi.on("session_start", async (event, ctx) => {
  const title = "Pi";
  const reason = event.reason ?? "startup";
  const body = `Session started (${reason})`;
  spawn_notify(title, body, ctx);
});"#,
        );
    }

    if has_session_shutdown {
        handlers.push_str(
            r#"
pi.on("session_shutdown", async (event, ctx) => {
  const title = "Pi";
  const reason = event.reason ?? "unknown";
  const body = `Session ended (${reason})`;
  spawn_notify(title, body, ctx);
});"#,
        );
    }

    if has_turn_end {
        handlers.push_str(
            r#"
pi.on("turn_end", async (event, ctx) => {
  const title = "Pi";
  const turnIndex = event.turnIndex ?? 0;
  const body = `Turn ${turnIndex} completed`;
  spawn_notify(title, body, ctx);
});"#,
        );
    }

    if has_tool_end {
        handlers.push_str(
            r#"
pi.on("tool_execution_end", async (event, ctx) => {
  const title = "Pi";
  const toolName = event.toolName ?? "Tool";
  const body = `${toolName} completed`;
  spawn_notify(title, body, ctx);
});"#,
        );
    }

    // Fallback: at least agent_end if nothing selected
    if handlers.is_empty() {
        handlers.push_str(
            r#"
pi.on("agent_end", async (event, ctx) => {
  const title = "Pi";
  const body = "Ready for input";
  spawn_notify(title, body, ctx);
});"#,
        );
    }

    let events_comment = events
        .iter()
        .map(|e| e.event_key())
        .collect::<Vec<_>>()
        .join(", ");

    // Build the extension string piece by piece to avoid format! conflicts
    // with JavaScript template literal ${...} syntax.
    let mut out = String::new();
    out.push_str("/**\n");
    out.push_str(" * pi-notification — Desktop notifications for Pi events\n");
    out.push_str(" * Installed by tlink install pi-notification\n");
    out.push_str(&format!(" * Selected events: {}\n", events_comment));
    out.push_str(" */\n\n");
    out.push_str("import type { ExtensionAPI } from \"@earendil-works/pi-coding-agent\";\n\n");
    out.push_str("/**\n");
    out.push_str(" * Capture tmux context and delegate to `tlink notify` for the actual\n");
    out.push_str(" * desktop notification.  Uses child_process.spawn with stdin pipe\n");
    out.push_str(" * (no shell escaping issues) to send the JSON payload.\n");
    out.push_str(" */\n");
    out.push_str("function spawn_notify(title: string, body: string, _ctx: any): void {\n");
    out.push_str("  const execSync = require(\"child_process\").execSync;\n");
    out.push_str("  const { spawn } = require(\"child_process\");\n");
    out.push_str("  let session = \"\", window = \"\", pane = \"\";\n");
    out.push_str("  try {\n");
    out.push_str("    session = execSync('tmux display-message -p \"#{session_name}\" 2>/dev/null', { encoding: \"utf8\" }).trim();\n");
    out.push_str("    window  = execSync('tmux display-message -p \"#{window_name}\" 2>/dev/null', { encoding: \"utf8\" }).trim();\n");
    out.push_str("    pane    = execSync('tmux display-message -p \"#{pane_index}\" 2>/dev/null', { encoding: \"utf8\" }).trim();\n");
    out.push_str("  } catch {\n");
    out.push_str("    session = \"no-tmux\";\n");
    out.push_str("    window  = \"0\";\n");
    out.push_str("    pane    = \"0\";\n");
    out.push_str("  }\n\n");
    out.push_str("  const payload = JSON.stringify({\n");
    out.push_str("    hook_event_name: \"Notification\",\n");
    out.push_str("    notification_type: \"idle_prompt\",\n");
    out.push_str("    message: body,\n");
    out.push_str("  });\n\n");
    out.push_str("  // Use spawn with stdin pipe — avoids shell escaping issues\n");
    out.push_str("  // with single quotes, backticks, etc. in the body text.\n");
    out.push_str("  const child = spawn(\"tlink\", [\"notify\", \"--session\", session, \"--window\", window, \"--pane\", pane], {\n");
    out.push_str("    stdio: [\"pipe\", \"pipe\", \"pipe\"],\n");
    out.push_str("  });\n");
    out.push_str("  child.stdin.write(payload);\n");
    out.push_str("  child.stdin.end();\n");
    out.push_str("}\n\n");
    out.push_str("export default function (pi: ExtensionAPI) {");
    out.push_str(&handlers);
    out.push_str("\n}\n");

    out
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extension_path_in_pi_dir() {
        let p = extension_path();
        assert!(p.to_string_lossy().contains(".pi/agent/extensions"));
        assert!(p.to_string_lossy().ends_with("pi-notification.ts"));
    }

    #[test]
    fn test_generate_extension_contains_handlers() {
        let ts = generate_extension(&[PiEvent::AgentEnd]);
        assert!(ts.contains("agent_end"));
        assert!(ts.contains("Ready for input"));
    }

    #[test]
    fn test_generate_extension_contains_multiple_handlers() {
        let ts = generate_extension(&[PiEvent::AgentEnd, PiEvent::SessionStart]);
        assert!(ts.contains("agent_end"));
        assert!(ts.contains("session_start"));
        assert!(ts.contains("Session started"));
    }

    #[test]
    fn test_generate_extension_empty_defaults_to_agent_end() {
        let ts = generate_extension(&[]);
        assert!(ts.contains("agent_end"));
    }

    #[test]
    fn test_generate_extension_has_correct_ts_syntax() {
        let ts = generate_extension(&[PiEvent::AgentEnd]);
        // Verify no Rust-format escape artifacts
        assert!(!ts.contains("{{"));
        assert!(!ts.contains("}}"));
        // Has valid TS structure
        assert!(ts.contains("import type { ExtensionAPI }"));
        assert!(ts.contains("export default function (pi: ExtensionAPI)"));
    }

    #[test]
    fn test_generate_extension_uses_spawn_not_shell_exec() {
        // Must use spawn with stdin pipe, not exec with shell commands
        let ts = generate_extension(&[PiEvent::AgentEnd]);
        assert!(ts.contains("spawn(\"tlink\""), "should use spawn");
        assert!(ts.contains("child.stdin.write"), "should pipe via stdin");
        assert!(ts.contains("child.stdin.end()"), "should close stdin");
        assert!(ts.contains("stdio: [\"pipe\""), "should use pipe stdio");
        // Should NOT use the old broken shell approach
        assert!(
            !ts.contains("exec(cmd"),
            "should not use exec with shell cmd"
        );
        assert!(!ts.contains("printf"), "should not use printf piping");
    }

    #[test]
    fn test_all_events_present() {
        let all = [
            PiEvent::AgentEnd,
            PiEvent::SessionStart,
            PiEvent::SessionShutdown,
            PiEvent::TurnEnd,
            PiEvent::ToolExecutionEnd,
        ];
        for e in &all {
            assert!(!e.event_key().is_empty());
            assert!(!e.label().is_empty());
            assert!(!e.description().is_empty());
        }
    }
}
