//! Per-agent hook payload adapters.
//!
//! Each coding agent (pi, claude, gemini, codex) has its own hook event format.
//! This module provides per-agent adapters that parse their native payloads and
//! return a human-readable title, message, and any action choices.
//!
//! The `resolve()` entry point routes to the correct adapter based on the
//! `source` field in the incoming JSON payload.

use serde::Deserialize;

/// Shared JSON payload from all agent hooks.
/// Fields are optional because each agent sends a different subset.
#[derive(Deserialize, Default, Debug)]
pub struct HookPayload {
    // ── source identification ──────────────────────────────────────────────
    pub source: Option<String>,

    // ── Claude Code fields ─────────────────────────────────────────────────
    pub hook_event_name: Option<String>,
    pub notification_type: Option<String>,
    pub message: Option<String>,
    pub error_type: Option<String>,
    pub tool_name: Option<String>,
    pub agent_type: Option<String>,
    pub task_title: Option<String>,
    pub reason: Option<String>,
    pub choices: Option<Vec<String>>,

    // ── Pi fields ──────────────────────────────────────────────────────────
    pub event: Option<String>,
    /// Tool name for tool_execution_end
    pub tool: Option<String>,
    /// Turn index for turn_end
    pub turn_index: Option<u64>,

    // ── Gemini fields ──────────────────────────────────────────────────────
    pub event_type: Option<String>,
    /// Task name for TaskCreated/TaskCompleted
    pub task_name: Option<String>,

    // ── Codex fields ───────────────────────────────────────────────────────
    pub status: Option<String>,
}

/// Resolved notification content: (title, message, button_choices).
pub type Resolved = (String, String, Vec<String>);

/// Trait for resolving an agent-specific hook payload into notification content.
pub trait HookPayloadAdapter: Send + Sync {
    /// Human-readable label for this agent (e.g., "pi", "Claude", "Gemini").
    #[allow(dead_code)]
    fn agent_label(&self) -> &str;
    /// Resolve a raw payload into (title, message, choices).
    fn resolve(&self, payload: &HookPayload) -> Resolved;
}

// ── Pi adapter ──────────────────────────────────────────────────────────────

pub struct PiHookAdapter;

impl HookPayloadAdapter for PiHookAdapter {
    fn agent_label(&self) -> &str {
        "pi"
    }

    fn resolve(&self, p: &HookPayload) -> Resolved {
        match p.event.as_deref() {
            Some("agent_end") => (
                title("pi", "Ready for input"),
                p.message
                    .clone()
                    .unwrap_or_else(|| "Pi finished responding".into()),
                vec![],
            ),
            Some("session_start") => (
                title("pi", "Session started"),
                p.message
                    .clone()
                    .unwrap_or_else(|| "A Pi session has started".into()),
                vec![],
            ),
            Some("session_shutdown") => (
                title("pi", "Session ended"),
                p.message
                    .clone()
                    .unwrap_or_else(|| "A Pi session has ended".into()),
                vec![],
            ),
            Some("turn_end") => (
                title("pi", "Turn completed"),
                p.turn_index.map_or_else(
                    || "Turn completed".into(),
                    |idx| format!("Turn {} completed", idx),
                ),
                vec![],
            ),
            Some("tool_execution_end") => (
                title("pi", "Tool completed"),
                p.tool.as_deref().map_or_else(
                    || "Tool execution finished".into(),
                    |t| format!("{} finished", t),
                ),
                vec![],
            ),
            _ => (
                title("pi", "Notification"),
                p.message
                    .clone()
                    .unwrap_or_else(|| "Pi notification".into()),
                vec![],
            ),
        }
    }
}

// ── Claude Code adapter ──────────────────────────────────────────────────────

pub struct ClaudeHookAdapter;

impl ClaudeHookAdapter {
    fn notif_type_title(t: &str) -> &'static str {
        match t {
            "idle_prompt" => "Waiting for your input",
            "permission_prompt" => "Permission needed",
            "auth_success" => "Authenticated",
            "elicitation_dialog" => "MCP: question for you",
            "elicitation_complete" => "MCP: dialog complete",
            "elicitation_response" => "MCP: response submitted",
            _ => "Notification",
        }
    }
}

impl HookPayloadAdapter for ClaudeHookAdapter {
    fn agent_label(&self) -> &str {
        "Claude"
    }

    fn resolve(&self, p: &HookPayload) -> Resolved {
        let prefix = "Claude";
        match p.hook_event_name.as_deref().unwrap_or("Notification") {
            "Notification" => {
                let nt = p.notification_type.as_deref();
                let t = nt.map_or("Notification", Self::notif_type_title);
                let m = p
                    .message
                    .clone()
                    .unwrap_or_else(|| "Claude notification".into());
                let choices = match nt {
                    Some("permission_prompt") => vec!["Allow".into(), "Deny".into()],
                    Some("elicitation_dialog") => p.choices.clone().unwrap_or_default(),
                    _ => vec![],
                };
                (title(prefix, t), m, choices)
            }
            "Stop" => (
                title(prefix, "Finished"),
                "Claude finished responding and is waiting for your input.".into(),
                vec![],
            ),
            "StopFailure" => (
                title(prefix, "Error"),
                format!(
                    "Turn failed: {}",
                    p.error_type.as_deref().unwrap_or("unknown error")
                ),
                vec![],
            ),
            "PostToolUse" => (
                title(prefix, "Tool completed"),
                format!("{} finished", p.tool_name.as_deref().unwrap_or("Tool")),
                vec![],
            ),
            "PostToolUseFailure" => (
                title(prefix, "Tool failed"),
                format!("{} error", p.tool_name.as_deref().unwrap_or("Tool")),
                vec![],
            ),
            "SubagentStop" => (
                title(prefix, "Subagent done"),
                format!(
                    "{} subagent finished",
                    p.agent_type.as_deref().unwrap_or("A")
                ),
                vec![],
            ),
            "TeammateIdle" => (
                title(prefix, "Teammate idle"),
                format!(
                    "{} is waiting for your input",
                    p.agent_type.as_deref().unwrap_or("A teammate")
                ),
                vec![],
            ),
            "TaskCreated" => (
                title(prefix, "Task created"),
                p.task_title.clone().unwrap_or_else(|| "New task".into()),
                vec![],
            ),
            "TaskCompleted" => (
                title(prefix, "Task complete"),
                "A task was marked as completed.".into(),
                vec![],
            ),
            "SessionStart" => (
                title(prefix, "Session started"),
                "A Claude Code session has started.".into(),
                vec![],
            ),
            "SessionEnd" => (
                title(prefix, "Session ended"),
                format!(
                    "Session ended: {}",
                    p.reason.as_deref().unwrap_or("unknown")
                ),
                vec![],
            ),
            other => (
                title(prefix, "Notification"),
                format!("{} event", other),
                vec![],
            ),
        }
    }
}

// ── Gemini adapter ──────────────────────────────────────────────────────────

pub struct GeminiHookAdapter;

impl HookPayloadAdapter for GeminiHookAdapter {
    fn agent_label(&self) -> &str {
        "Gemini"
    }

    fn resolve(&self, p: &HookPayload) -> Resolved {
        let prefix = "Gemini";
        // Gemini uses either event (hook key) or notification_type fields
        let event = p
            .event
            .as_deref()
            .or(p.hook_event_name.as_deref())
            .or(p.event_type.as_deref());
        let nt = p.notification_type.as_deref();

        match event {
            Some("AfterAgent") | Some("Agent") | Some("after_agent") => (
                title(prefix, "Finished"),
                p.message
                    .clone()
                    .unwrap_or_else(|| "Gemini finished responding".into()),
                vec![],
            ),
            Some("SessionStart") | Some("session_start") => (
                title(prefix, "Session started"),
                p.message
                    .clone()
                    .unwrap_or_else(|| "A Gemini session has started".into()),
                vec![],
            ),
            Some("SessionEnd") | Some("session_end") | Some("SessionShutdown") => (
                title(prefix, "Session ended"),
                p.message
                    .clone()
                    .unwrap_or_else(|| "A Gemini session has ended".into()),
                vec![],
            ),
            Some("TaskCreated") | Some("task_created") => (
                title(prefix, "Task created"),
                p.task_name
                    .clone()
                    .or(p.task_title.clone())
                    .or(p.message.clone())
                    .unwrap_or_else(|| "New task".into()),
                vec![],
            ),
            Some("TaskCompleted") | Some("task_completed") => (
                title(prefix, "Task complete"),
                p.message
                    .clone()
                    .unwrap_or_else(|| "A task was completed".into()),
                vec![],
            ),
            Some("BeforeTool") | Some("before_tool") => (
                title(prefix, "Tool starting"),
                p.tool_name.clone().map_or_else(
                    || "A tool is about to run".into(),
                    |t| format!("{} starting", t),
                ),
                vec![],
            ),
            Some("AfterTool") | Some("after_tool") => (
                title(prefix, "Tool completed"),
                p.tool_name
                    .clone()
                    .map_or_else(|| "Tool finished".into(), |t| format!("{} finished", t)),
                vec![],
            ),
            // Fallback: check notification_type for idle_prompt style
            _ => match nt {
                Some("idle_prompt") => (
                    title(prefix, "Waiting for your input"),
                    p.message
                        .clone()
                        .unwrap_or_else(|| "Gemini needs your input".into()),
                    vec![],
                ),
                _ => (
                    title(prefix, "Notification"),
                    p.message
                        .clone()
                        .or(p.task_name.clone())
                        .unwrap_or_else(|| "Gemini notification".into()),
                    vec![],
                ),
            },
        }
    }
}

// ── Codex adapter ───────────────────────────────────────────────────────────

pub struct CodexHookAdapter;

impl HookPayloadAdapter for CodexHookAdapter {
    fn agent_label(&self) -> &str {
        "Codex"
    }

    fn resolve(&self, p: &HookPayload) -> Resolved {
        let prefix = "Codex";
        // Codex calls the notify script with "turn-ended" argument.
        // Check status first — some versions may also include a message.
        if let Some(ref status) = p.status {
            return match status.as_str() {
                "turn-ended" | "complete" => {
                    let msg = p
                        .message
                        .clone()
                        .unwrap_or_else(|| "Codex finished the current task.".into());
                    (title(prefix, "Task complete"), msg, vec![])
                }
                "error" => (
                    title(prefix, "Error"),
                    p.message
                        .clone()
                        .unwrap_or_else(|| "Codex encountered an error.".into()),
                    vec![],
                ),
                _ => (
                    title(prefix, "Notification"),
                    format!("Codex status: {}", status),
                    vec![],
                ),
            };
        }
        // Fallback: message-only payload
        if let Some(ref msg) = p.message {
            if !msg.is_empty() {
                return (title(prefix, "Task complete"), msg.clone(), vec![]);
            }
        }
        // Default: basic turn-ended notification
        (
            title(prefix, "Task complete"),
            "Codex finished the current task.".into(),
            vec![],
        )
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Build a title with agent prefix, e.g. `[pi] Waiting for your input`.
fn title(agent: &str, event: &str) -> String {
    format!("[{}] {}", agent, event)
}

/// Route to the correct adapter based on payload source.
/// Falls back to Claude adapter when source is missing or unknown.
pub fn adapter_for(payload: &HookPayload) -> Box<dyn HookPayloadAdapter> {
    match payload.source.as_deref() {
        Some("pi") => Box::new(PiHookAdapter),
        Some("claude") => Box::new(ClaudeHookAdapter),
        Some("gemini") => Box::new(GeminiHookAdapter),
        Some("codex") => Box::new(CodexHookAdapter),
        // Default: Claude for backward compatibility
        _ => Box::new(ClaudeHookAdapter),
    }
}

/// Resolve a payload using the appropriate adapter.
pub fn resolve(payload: &HookPayload) -> Resolved {
    adapter_for(payload).resolve(payload)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn p() -> HookPayload {
        HookPayload::default()
    }

    // ── PiHookAdapter ──────────────────────────────────────────────────────

    #[test]
    fn pi_agent_end() {
        let pl = HookPayload {
            source: Some("pi".into()),
            event: Some("agent_end".into()),
            message: Some("Done!".into()),
            ..p()
        };
        let (title, msg, choices) = PiHookAdapter.resolve(&pl);
        assert_eq!(title, "[pi] Ready for input");
        assert_eq!(msg, "Done!");
        assert!(choices.is_empty());
    }

    #[test]
    fn pi_agent_end_no_message() {
        let pl = HookPayload {
            source: Some("pi".into()),
            event: Some("agent_end".into()),
            ..p()
        };
        let (title, msg, _) = PiHookAdapter.resolve(&pl);
        assert_eq!(title, "[pi] Ready for input");
        assert_eq!(msg, "Pi finished responding");
    }

    #[test]
    fn pi_session_start() {
        let pl = HookPayload {
            source: Some("pi".into()),
            event: Some("session_start".into()),
            ..p()
        };
        let (title, msg, _) = PiHookAdapter.resolve(&pl);
        assert_eq!(title, "[pi] Session started");
        assert_eq!(msg, "A Pi session has started");
    }

    #[test]
    fn pi_session_shutdown() {
        let pl = HookPayload {
            source: Some("pi".into()),
            event: Some("session_shutdown".into()),
            ..p()
        };
        let (title, msg, _) = PiHookAdapter.resolve(&pl);
        assert_eq!(title, "[pi] Session ended");
        assert_eq!(msg, "A Pi session has ended");
    }

    #[test]
    fn pi_turn_end_with_index() {
        let pl = HookPayload {
            source: Some("pi".into()),
            event: Some("turn_end".into()),
            turn_index: Some(3),
            ..p()
        };
        let (title, msg, _) = PiHookAdapter.resolve(&pl);
        assert_eq!(title, "[pi] Turn completed");
        assert_eq!(msg, "Turn 3 completed");
    }

    #[test]
    fn pi_turn_end_no_index() {
        let pl = HookPayload {
            source: Some("pi".into()),
            event: Some("turn_end".into()),
            ..p()
        };
        let (title, msg, _) = PiHookAdapter.resolve(&pl);
        assert_eq!(title, "[pi] Turn completed");
        assert_eq!(msg, "Turn completed");
    }

    #[test]
    fn pi_tool_execution_end_with_name() {
        let pl = HookPayload {
            source: Some("pi".into()),
            event: Some("tool_execution_end".into()),
            tool: Some("Bash".into()),
            ..p()
        };
        let (title, msg, _) = PiHookAdapter.resolve(&pl);
        assert_eq!(title, "[pi] Tool completed");
        assert_eq!(msg, "Bash finished");
    }

    #[test]
    fn pi_tool_execution_end_no_name() {
        let pl = HookPayload {
            source: Some("pi".into()),
            event: Some("tool_execution_end".into()),
            ..p()
        };
        let (title, msg, _) = PiHookAdapter.resolve(&pl);
        assert_eq!(title, "[pi] Tool completed");
        assert_eq!(msg, "Tool execution finished");
    }

    #[test]
    fn pi_unknown_event_fallback() {
        let pl = HookPayload {
            source: Some("pi".into()),
            event: Some("future_event".into()),
            message: Some("Hello".into()),
            ..p()
        };
        let (title, msg, _) = PiHookAdapter.resolve(&pl);
        assert_eq!(title, "[pi] Notification");
        assert_eq!(msg, "Hello");
    }

    #[test]
    fn pi_no_event_fallback() {
        let pl = HookPayload {
            source: Some("pi".into()),
            ..p()
        };
        let (title, msg, _) = PiHookAdapter.resolve(&pl);
        assert_eq!(title, "[pi] Notification");
        assert_eq!(msg, "Pi notification");
    }

    #[test]
    fn pi_agent_label() {
        assert_eq!(PiHookAdapter.agent_label(), "pi");
    }

    // ── ClaudeHookAdapter ─────────────────────────────────────────────────

    #[test]
    fn claude_notification_idle_prompt() {
        let pl = HookPayload {
            source: Some("claude".into()),
            hook_event_name: Some("Notification".into()),
            notification_type: Some("idle_prompt".into()),
            message: Some("Done".into()),
            ..p()
        };
        let (title, msg, _) = ClaudeHookAdapter.resolve(&pl);
        assert_eq!(title, "[Claude] Waiting for your input");
        assert_eq!(msg, "Done");
    }

    #[test]
    fn claude_notification_permission_prompt_choices() {
        let pl = HookPayload {
            source: Some("claude".into()),
            hook_event_name: Some("Notification".into()),
            notification_type: Some("permission_prompt".into()),
            message: Some("Allow?".into()),
            ..p()
        };
        let (title, _msg, choices) = ClaudeHookAdapter.resolve(&pl);
        assert_eq!(title, "[Claude] Permission needed");
        assert_eq!(choices, vec!["Allow", "Deny"]);
    }

    #[test]
    fn claude_notification_elicitation_dialog_choices() {
        let pl = HookPayload {
            source: Some("claude".into()),
            hook_event_name: Some("Notification".into()),
            notification_type: Some("elicitation_dialog".into()),
            choices: Some(vec!["Yes".into(), "No".into()]),
            ..p()
        };
        let (_title, _msg, choices) = ClaudeHookAdapter.resolve(&pl);
        assert_eq!(choices, vec!["Yes", "No"]);
    }

    #[test]
    fn claude_notification_auth_success() {
        let pl = HookPayload {
            source: Some("claude".into()),
            hook_event_name: Some("Notification".into()),
            notification_type: Some("auth_success".into()),
            message: Some("Token refreshed".into()),
            ..p()
        };
        let (title, _msg, _) = ClaudeHookAdapter.resolve(&pl);
        assert_eq!(title, "[Claude] Authenticated");
    }

    #[test]
    fn claude_notification_elicitation_complete() {
        let pl = HookPayload {
            source: Some("claude".into()),
            hook_event_name: Some("Notification".into()),
            notification_type: Some("elicitation_complete".into()),
            ..p()
        };
        let (title, _msg, _) = ClaudeHookAdapter.resolve(&pl);
        assert_eq!(title, "[Claude] MCP: dialog complete");
    }

    #[test]
    fn claude_notification_elicitation_response() {
        let pl = HookPayload {
            source: Some("claude".into()),
            hook_event_name: Some("Notification".into()),
            notification_type: Some("elicitation_response".into()),
            ..p()
        };
        let (title, _msg, _) = ClaudeHookAdapter.resolve(&pl);
        assert_eq!(title, "[Claude] MCP: response submitted");
    }

    #[test]
    fn claude_notification_unknown_type() {
        let pl = HookPayload {
            source: Some("claude".into()),
            hook_event_name: Some("Notification".into()),
            notification_type: Some("unknown_nt".into()),
            message: Some("Hello".into()),
            ..p()
        };
        let (title, msg, _) = ClaudeHookAdapter.resolve(&pl);
        assert_eq!(title, "[Claude] Notification");
        assert_eq!(msg, "Hello");
    }

    #[test]
    fn claude_notification_no_type_default_message() {
        let pl = HookPayload {
            source: Some("claude".into()),
            hook_event_name: Some("Notification".into()),
            ..p()
        };
        let (title, msg, _) = ClaudeHookAdapter.resolve(&pl);
        assert_eq!(title, "[Claude] Notification");
        assert_eq!(msg, "Claude notification");
    }

    #[test]
    fn claude_stop() {
        let pl = HookPayload {
            source: Some("claude".into()),
            hook_event_name: Some("Stop".into()),
            ..p()
        };
        let (title, msg, choices) = ClaudeHookAdapter.resolve(&pl);
        assert_eq!(title, "[Claude] Finished");
        assert!(msg.contains("waiting for your input"));
        assert!(choices.is_empty());
    }

    #[test]
    fn claude_stop_failure() {
        let pl = HookPayload {
            source: Some("claude".into()),
            hook_event_name: Some("StopFailure".into()),
            error_type: Some("timeout".into()),
            ..p()
        };
        let (title, msg, _) = ClaudeHookAdapter.resolve(&pl);
        assert_eq!(title, "[Claude] Error");
        assert!(msg.contains("timeout"));
    }

    #[test]
    fn claude_stop_failure_no_error_type() {
        let pl = HookPayload {
            source: Some("claude".into()),
            hook_event_name: Some("StopFailure".into()),
            ..p()
        };
        let (_title, msg, _) = ClaudeHookAdapter.resolve(&pl);
        assert!(msg.contains("unknown error"));
    }

    #[test]
    fn claude_post_tool_use() {
        let pl = HookPayload {
            source: Some("claude".into()),
            hook_event_name: Some("PostToolUse".into()),
            tool_name: Some("Bash".into()),
            ..p()
        };
        let (title, msg, _) = ClaudeHookAdapter.resolve(&pl);
        assert_eq!(title, "[Claude] Tool completed");
        assert!(msg.contains("Bash"));
    }

    #[test]
    fn claude_post_tool_use_no_name() {
        let pl = HookPayload {
            source: Some("claude".into()),
            hook_event_name: Some("PostToolUse".into()),
            ..p()
        };
        let (title, msg, _) = ClaudeHookAdapter.resolve(&pl);
        assert_eq!(title, "[Claude] Tool completed");
        assert!(msg.contains("Tool"));
    }

    #[test]
    fn claude_post_tool_use_failure() {
        let pl = HookPayload {
            source: Some("claude".into()),
            hook_event_name: Some("PostToolUseFailure".into()),
            tool_name: Some("Edit".into()),
            ..p()
        };
        let (title, msg, _) = ClaudeHookAdapter.resolve(&pl);
        assert_eq!(title, "[Claude] Tool failed");
        assert!(msg.contains("Edit"));
    }

    #[test]
    fn claude_subagent_stop() {
        let pl = HookPayload {
            source: Some("claude".into()),
            hook_event_name: Some("SubagentStop".into()),
            agent_type: Some("researcher".into()),
            ..p()
        };
        let (title, msg, _) = ClaudeHookAdapter.resolve(&pl);
        assert_eq!(title, "[Claude] Subagent done");
        assert!(msg.contains("researcher"));
    }

    #[test]
    fn claude_subagent_stop_no_type() {
        let pl = HookPayload {
            source: Some("claude".into()),
            hook_event_name: Some("SubagentStop".into()),
            ..p()
        };
        let (_title, msg, _) = ClaudeHookAdapter.resolve(&pl);
        assert!(msg.contains("A subagent finished"));
    }

    #[test]
    fn claude_teammate_idle() {
        let pl = HookPayload {
            source: Some("claude".into()),
            hook_event_name: Some("TeammateIdle".into()),
            agent_type: Some("Alice".into()),
            ..p()
        };
        let (title, msg, _) = ClaudeHookAdapter.resolve(&pl);
        assert_eq!(title, "[Claude] Teammate idle");
        assert!(msg.contains("Alice"));
    }

    #[test]
    fn claude_teammate_idle_no_type() {
        let pl = HookPayload {
            source: Some("claude".into()),
            hook_event_name: Some("TeammateIdle".into()),
            ..p()
        };
        let (_title, msg, _) = ClaudeHookAdapter.resolve(&pl);
        assert!(msg.contains("A teammate"));
    }

    #[test]
    fn claude_task_created() {
        let pl = HookPayload {
            source: Some("claude".into()),
            hook_event_name: Some("TaskCreated".into()),
            task_title: Some("Write tests".into()),
            ..p()
        };
        let (title, msg, _) = ClaudeHookAdapter.resolve(&pl);
        assert_eq!(title, "[Claude] Task created");
        assert_eq!(msg, "Write tests");
    }

    #[test]
    fn claude_task_created_no_title() {
        let pl = HookPayload {
            source: Some("claude".into()),
            hook_event_name: Some("TaskCreated".into()),
            ..p()
        };
        let (_title, msg, _) = ClaudeHookAdapter.resolve(&pl);
        assert_eq!(msg, "New task");
    }

    #[test]
    fn claude_task_completed() {
        let pl = HookPayload {
            source: Some("claude".into()),
            hook_event_name: Some("TaskCompleted".into()),
            ..p()
        };
        let (title, msg, _) = ClaudeHookAdapter.resolve(&pl);
        assert_eq!(title, "[Claude] Task complete");
        assert!(msg.contains("completed"));
    }

    #[test]
    fn claude_session_start() {
        let pl = HookPayload {
            source: Some("claude".into()),
            hook_event_name: Some("SessionStart".into()),
            ..p()
        };
        let (title, msg, _) = ClaudeHookAdapter.resolve(&pl);
        assert_eq!(title, "[Claude] Session started");
        assert!(msg.contains("session has started"));
    }

    #[test]
    fn claude_session_end() {
        let pl = HookPayload {
            source: Some("claude".into()),
            hook_event_name: Some("SessionEnd".into()),
            reason: Some("timeout".into()),
            ..p()
        };
        let (title, msg, _) = ClaudeHookAdapter.resolve(&pl);
        assert_eq!(title, "[Claude] Session ended");
        assert!(msg.contains("timeout"));
    }

    #[test]
    fn claude_session_end_no_reason() {
        let pl = HookPayload {
            source: Some("claude".into()),
            hook_event_name: Some("SessionEnd".into()),
            ..p()
        };
        let (_title, msg, _) = ClaudeHookAdapter.resolve(&pl);
        assert!(msg.contains("unknown"));
    }

    #[test]
    fn claude_unknown_event() {
        let pl = HookPayload {
            source: Some("claude".into()),
            hook_event_name: Some("FutureEvent".into()),
            ..p()
        };
        let (title, msg, _) = ClaudeHookAdapter.resolve(&pl);
        assert_eq!(title, "[Claude] Notification");
        assert!(msg.contains("FutureEvent"));
    }

    #[test]
    fn claude_no_event_name_treated_as_notification() {
        let pl = HookPayload {
            source: Some("claude".into()),
            notification_type: Some("idle_prompt".into()),
            message: Some("Hi".into()),
            ..p()
        };
        let (title, _msg, _) = ClaudeHookAdapter.resolve(&pl);
        assert_eq!(title, "[Claude] Waiting for your input");
    }

    #[test]
    fn claude_agent_label() {
        assert_eq!(ClaudeHookAdapter.agent_label(), "Claude");
    }

    // ── GeminiHookAdapter ──────────────────────────────────────────────────

    #[test]
    fn gemini_after_agent() {
        let pl = HookPayload {
            source: Some("gemini".into()),
            event: Some("AfterAgent".into()),
            message: Some("Done!".into()),
            ..p()
        };
        let (title, msg, _) = GeminiHookAdapter.resolve(&pl);
        assert_eq!(title, "[Gemini] Finished");
        assert_eq!(msg, "Done!");
    }

    #[test]
    fn gemini_after_agent_no_message() {
        let pl = HookPayload {
            source: Some("gemini".into()),
            event: Some("AfterAgent".into()),
            ..p()
        };
        let (title, msg, _) = GeminiHookAdapter.resolve(&pl);
        assert_eq!(title, "[Gemini] Finished");
        assert_eq!(msg, "Gemini finished responding");
    }

    #[test]
    fn gemini_session_start() {
        let pl = HookPayload {
            source: Some("gemini".into()),
            event: Some("SessionStart".into()),
            ..p()
        };
        let (title, msg, _) = GeminiHookAdapter.resolve(&pl);
        assert_eq!(title, "[Gemini] Session started");
        assert_eq!(msg, "A Gemini session has started");
    }

    #[test]
    fn gemini_session_end() {
        let pl = HookPayload {
            source: Some("gemini".into()),
            event: Some("SessionEnd".into()),
            ..p()
        };
        let (title, msg, _) = GeminiHookAdapter.resolve(&pl);
        assert_eq!(title, "[Gemini] Session ended");
        assert_eq!(msg, "A Gemini session has ended");
    }

    #[test]
    fn gemini_task_created() {
        let pl = HookPayload {
            source: Some("gemini".into()),
            event: Some("TaskCreated".into()),
            task_name: Some("Add auth".into()),
            ..p()
        };
        let (title, msg, _) = GeminiHookAdapter.resolve(&pl);
        assert_eq!(title, "[Gemini] Task created");
        assert_eq!(msg, "Add auth");
    }

    #[test]
    fn gemini_task_completed() {
        let pl = HookPayload {
            source: Some("gemini".into()),
            event: Some("TaskCompleted".into()),
            message: Some("All done".into()),
            ..p()
        };
        let (title, msg, _) = GeminiHookAdapter.resolve(&pl);
        assert_eq!(title, "[Gemini] Task complete");
        assert_eq!(msg, "All done");
    }

    #[test]
    fn gemini_before_tool() {
        let pl = HookPayload {
            source: Some("gemini".into()),
            event: Some("BeforeTool".into()),
            tool_name: Some("Bash".into()),
            ..p()
        };
        let (title, msg, _) = GeminiHookAdapter.resolve(&pl);
        assert_eq!(title, "[Gemini] Tool starting");
        assert_eq!(msg, "Bash starting");
    }

    #[test]
    fn gemini_after_tool() {
        let pl = HookPayload {
            source: Some("gemini".into()),
            event: Some("AfterTool".into()),
            tool_name: Some("Write".into()),
            ..p()
        };
        let (title, msg, _) = GeminiHookAdapter.resolve(&pl);
        assert_eq!(title, "[Gemini] Tool completed");
        assert_eq!(msg, "Write finished");
    }

    #[test]
    fn gemini_after_tool_no_name() {
        let pl = HookPayload {
            source: Some("gemini".into()),
            event: Some("AfterTool".into()),
            ..p()
        };
        let (title, msg, _) = GeminiHookAdapter.resolve(&pl);
        assert_eq!(title, "[Gemini] Tool completed");
        assert_eq!(msg, "Tool finished");
    }

    #[test]
    fn gemini_hook_event_name_alias() {
        let pl = HookPayload {
            source: Some("gemini".into()),
            hook_event_name: Some("AfterAgent".into()),
            message: Some("Done".into()),
            ..p()
        };
        let (title, msg, _) = GeminiHookAdapter.resolve(&pl);
        assert_eq!(title, "[Gemini] Finished");
        assert_eq!(msg, "Done");
    }

    #[test]
    fn gemini_event_type_alias() {
        let pl = HookPayload {
            source: Some("gemini".into()),
            event_type: Some("Agent".into()),
            ..p()
        };
        let (title, _msg, _) = GeminiHookAdapter.resolve(&pl);
        assert_eq!(title, "[Gemini] Finished");
    }

    #[test]
    fn gemini_idle_prompt_fallback() {
        let pl = HookPayload {
            source: Some("gemini".into()),
            notification_type: Some("idle_prompt".into()),
            message: Some("Need input".into()),
            ..p()
        };
        let (title, msg, _) = GeminiHookAdapter.resolve(&pl);
        assert_eq!(title, "[Gemini] Waiting for your input");
        assert_eq!(msg, "Need input");
    }

    #[test]
    fn gemini_unknown_event_fallback() {
        let pl = HookPayload {
            source: Some("gemini".into()),
            ..p()
        };
        let (title, msg, _) = GeminiHookAdapter.resolve(&pl);
        assert_eq!(title, "[Gemini] Notification");
        assert_eq!(msg, "Gemini notification");
    }

    #[test]
    fn gemini_agent_label() {
        assert_eq!(GeminiHookAdapter.agent_label(), "Gemini");
    }

    // ── CodexHookAdapter ───────────────────────────────────────────────────

    #[test]
    fn codex_default() {
        let pl = HookPayload {
            source: Some("codex".into()),
            ..p()
        };
        let (title, msg, _) = CodexHookAdapter.resolve(&pl);
        assert_eq!(title, "[Codex] Task complete");
        assert_eq!(msg, "Codex finished the current task.");
    }

    #[test]
    fn codex_with_message() {
        let pl = HookPayload {
            source: Some("codex".into()),
            message: Some("Build succeeded".into()),
            ..p()
        };
        let (title, msg, _) = CodexHookAdapter.resolve(&pl);
        assert_eq!(title, "[Codex] Task complete");
        assert_eq!(msg, "Build succeeded");
    }

    #[test]
    fn codex_turn_ended_status() {
        let pl = HookPayload {
            source: Some("codex".into()),
            status: Some("turn-ended".into()),
            ..p()
        };
        let (title, msg, _) = CodexHookAdapter.resolve(&pl);
        assert_eq!(title, "[Codex] Task complete");
        assert_eq!(msg, "Codex finished the current task.");
    }

    #[test]
    fn codex_error_status() {
        let pl = HookPayload {
            source: Some("codex".into()),
            status: Some("error".into()),
            message: Some("Build failed".into()),
            ..p()
        };
        let (title, msg, _) = CodexHookAdapter.resolve(&pl);
        assert_eq!(title, "[Codex] Error");
        assert_eq!(msg, "Build failed");
    }

    #[test]
    fn codex_error_status_no_message() {
        let pl = HookPayload {
            source: Some("codex".into()),
            status: Some("error".into()),
            ..p()
        };
        let (title, msg, _) = CodexHookAdapter.resolve(&pl);
        assert_eq!(title, "[Codex] Error");
        assert_eq!(msg, "Codex encountered an error.");
    }

    #[test]
    fn codex_unknown_status() {
        let pl = HookPayload {
            source: Some("codex".into()),
            status: Some("started".into()),
            ..p()
        };
        let (title, msg, _) = CodexHookAdapter.resolve(&pl);
        assert_eq!(title, "[Codex] Notification");
        assert_eq!(msg, "Codex status: started");
    }

    #[test]
    fn codex_agent_label() {
        assert_eq!(CodexHookAdapter.agent_label(), "Codex");
    }

    // ── adapter_for ────────────────────────────────────────────────────────

    #[test]
    fn adapter_for_pi() {
        let pl = HookPayload {
            source: Some("pi".into()),
            ..p()
        };
        assert_eq!(adapter_for(&pl).agent_label(), "pi");
    }

    #[test]
    fn adapter_for_claude() {
        let pl = HookPayload {
            source: Some("claude".into()),
            ..p()
        };
        assert_eq!(adapter_for(&pl).agent_label(), "Claude");
    }

    #[test]
    fn adapter_for_gemini() {
        let pl = HookPayload {
            source: Some("gemini".into()),
            ..p()
        };
        assert_eq!(adapter_for(&pl).agent_label(), "Gemini");
    }

    #[test]
    fn adapter_for_codex() {
        let pl = HookPayload {
            source: Some("codex".into()),
            ..p()
        };
        assert_eq!(adapter_for(&pl).agent_label(), "Codex");
    }

    #[test]
    fn adapter_for_unknown_falls_back_to_claude() {
        let pl = HookPayload {
            source: Some("unknown_agent".into()),
            ..p()
        };
        assert_eq!(adapter_for(&pl).agent_label(), "Claude");
    }

    #[test]
    fn adapter_for_none_falls_back_to_claude() {
        let pl = HookPayload::default();
        assert_eq!(adapter_for(&pl).agent_label(), "Claude");
    }

    // ── resolve ────────────────────────────────────────────────────────────

    #[test]
    fn resolve_routes_to_correct_adapter() {
        let pl = HookPayload {
            source: Some("pi".into()),
            event: Some("agent_end".into()),
            ..p()
        };
        let (title, _msg, _) = resolve(&pl);
        assert_eq!(title, "[pi] Ready for input");
    }

    #[test]
    fn resolve_no_source_falls_back_to_claude() {
        let pl = HookPayload {
            hook_event_name: Some("Stop".into()),
            ..p()
        };
        let (title, _msg, _) = resolve(&pl);
        assert_eq!(title, "[Claude] Finished");
    }

    // ── title helper ───────────────────────────────────────────────────────

    #[test]
    fn title_formats_correctly() {
        assert_eq!(title("pi", "Ready for input"), "[pi] Ready for input");
        assert_eq!(title("Claude", "Finished"), "[Claude] Finished");
        assert_eq!(
            title("Gemini", "Session started"),
            "[Gemini] Session started"
        );
        assert_eq!(title("Codex", "Task complete"), "[Codex] Task complete");
    }
}
