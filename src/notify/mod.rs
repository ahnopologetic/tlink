mod adapter;
mod dunstify;
mod notify_send;
mod osascript;
mod terminal_notifier;
pub mod utils;

pub use adapter::{NotificationAdapter, NotificationRequest};

use anyhow::Result;
use serde::Deserialize;
use std::io::Read;

#[derive(Deserialize, Default)]
struct Payload {
    hook_event_name: Option<String>,
    notification_type: Option<String>,
    message: Option<String>,
    error_type: Option<String>,
    tool_name: Option<String>,
    agent_type: Option<String>,
    task_title: Option<String>,
    reason: Option<String>,
    choices: Option<Vec<String>>,
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

fn resolve(payload: &Payload) -> (String, String, Vec<String>) {
    match payload.hook_event_name.as_deref().unwrap_or("Notification") {
        "Notification" => {
            let notification_type = payload.notification_type.as_deref();
            let t = notification_type
                .map(type_to_title)
                .unwrap_or("Claude Code");
            let m = payload
                .message
                .clone()
                .unwrap_or_else(|| "Claude notification".into());
            let choices = match notification_type {
                Some("permission_prompt") => vec!["Allow".into(), "Deny".into()],
                Some("elicitation_dialog") => payload.choices.clone().unwrap_or_default(),
                _ => vec![],
            };
            (t.into(), m, choices)
        }
        "Stop" => (
            "Claude finished".into(),
            "Claude finished responding and is waiting for your input.".into(),
            vec![],
        ),
        "StopFailure" => (
            "Claude error".into(),
            format!(
                "Turn failed: {}",
                payload.error_type.as_deref().unwrap_or("unknown error")
            ),
            vec![],
        ),
        "PostToolUse" => (
            "Tool completed".into(),
            format!(
                "{} finished",
                payload.tool_name.as_deref().unwrap_or("Tool")
            ),
            vec![],
        ),
        "PostToolUseFailure" => (
            "Tool failed".into(),
            format!("{} error", payload.tool_name.as_deref().unwrap_or("Tool")),
            vec![],
        ),
        "SubagentStop" => (
            "Subagent done".into(),
            format!(
                "{} subagent finished",
                payload.agent_type.as_deref().unwrap_or("A")
            ),
            vec![],
        ),
        "TeammateIdle" => (
            "Teammate idle".into(),
            format!(
                "{} is waiting for your input",
                payload.agent_type.as_deref().unwrap_or("Teammate")
            ),
            vec![],
        ),
        "TaskCreated" => (
            "Task created".into(),
            payload
                .task_title
                .clone()
                .unwrap_or_else(|| "New task".into()),
            vec![],
        ),
        "TaskCompleted" => (
            "Task complete".into(),
            "A task was marked as completed.".into(),
            vec![],
        ),
        "SessionStart" => (
            "Session started".into(),
            "A Claude Code session has started.".into(),
            vec![],
        ),
        "SessionEnd" => (
            "Session ended".into(),
            format!(
                "Session ended: {}",
                payload.reason.as_deref().unwrap_or("unknown")
            ),
            vec![],
        ),
        other => ("Claude Code".into(), format!("{} event", other), vec![]),
    }
}

pub fn make_adapter(method: &str) -> Box<dyn NotificationAdapter> {
    match method {
        "terminal-notifier" => Box::new(terminal_notifier::TerminalNotifierAdapter),
        "dunstify" => Box::new(dunstify::DunstifyAdapter),
        "notify-send" => Box::new(notify_send::NotifySendAdapter),
        _ => Box::new(osascript::OsascriptAdapter),
    }
}

pub fn run(session: &str, window: &str, pane: &str) -> Result<()> {
    let mut stdin = String::new();
    std::io::stdin().read_to_string(&mut stdin)?;

    let payload: Payload = serde_json::from_str(&stdin).unwrap_or_default();
    let (title, message, _choices) = resolve(&payload);

    let deeplink = format!("tmux://{}/{}/{}", session, window, pane);
    let location = format!("{} > {} > {}", session, window, pane);

    let config = crate::config::load().unwrap_or_default();
    let method = config.notification_method.as_deref().unwrap_or("osascript");

    let req = NotificationRequest {
        title,
        message,
        location,
        deeplink,
    };

    make_adapter(method).notify(&req)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── type_to_title ──────────────────────────────────────────────────────────

    #[test]
    fn type_to_title_all_known_types() {
        assert_eq!(type_to_title("idle_prompt"), "Waiting for your input");
        assert_eq!(type_to_title("permission_prompt"), "Permission needed");
        assert_eq!(type_to_title("auth_success"), "Authenticated");
        assert_eq!(type_to_title("elicitation_dialog"), "MCP: question for you");
        assert_eq!(
            type_to_title("elicitation_complete"),
            "MCP: dialog complete"
        );
        assert_eq!(
            type_to_title("elicitation_response"),
            "MCP: response submitted"
        );
    }

    #[test]
    fn type_to_title_unknown_falls_back() {
        assert_eq!(type_to_title("unknown_type"), "Claude Code");
        assert_eq!(type_to_title(""), "Claude Code");
    }

    // ── Payload deserialization ────────────────────────────────────────────────

    #[test]
    fn payload_deserializes_message_and_type() {
        let json = r#"{"notification_type":"idle_prompt","message":"Done!"}"#;
        let p: Payload = serde_json::from_str(json).unwrap();
        assert_eq!(p.notification_type.as_deref(), Some("idle_prompt"));
        assert_eq!(p.message.as_deref(), Some("Done!"));
    }

    #[test]
    fn payload_missing_fields_default() {
        let p: Payload = serde_json::from_str("{}").unwrap_or_default();
        assert!(p.notification_type.is_none());
        assert!(p.message.is_none());
    }

    // ── resolve() — all hook_event_name branches ───────────────────────────────

    fn payload_with_event(event: &str) -> Payload {
        Payload {
            hook_event_name: Some(event.into()),
            ..Default::default()
        }
    }

    #[test]
    fn resolve_notification_idle_prompt() {
        let p = Payload {
            hook_event_name: Some("Notification".into()),
            notification_type: Some("idle_prompt".into()),
            message: Some("Done".into()),
            ..Default::default()
        };
        let (title, msg, choices) = resolve(&p);
        assert_eq!(title, "Waiting for your input");
        assert_eq!(msg, "Done");
        assert!(choices.is_empty());
    }

    #[test]
    fn resolve_notification_permission_prompt_has_choices() {
        let p = Payload {
            hook_event_name: Some("Notification".into()),
            notification_type: Some("permission_prompt".into()),
            message: Some("Allow?".into()),
            ..Default::default()
        };
        let (_, _, choices) = resolve(&p);
        assert_eq!(choices, vec!["Allow", "Deny"]);
    }

    #[test]
    fn resolve_notification_elicitation_dialog_uses_payload_choices() {
        let p = Payload {
            hook_event_name: Some("Notification".into()),
            notification_type: Some("elicitation_dialog".into()),
            choices: Some(vec!["Yes".into(), "No".into()]),
            ..Default::default()
        };
        let (_, _, choices) = resolve(&p);
        assert_eq!(choices, vec!["Yes", "No"]);
    }

    #[test]
    fn resolve_notification_elicitation_dialog_no_choices_empty() {
        let p = Payload {
            hook_event_name: Some("Notification".into()),
            notification_type: Some("elicitation_dialog".into()),
            choices: None,
            ..Default::default()
        };
        let (_, _, choices) = resolve(&p);
        assert!(choices.is_empty());
    }

    #[test]
    fn resolve_notification_default_message_fallback() {
        let p = Payload {
            hook_event_name: Some("Notification".into()),
            notification_type: None,
            ..Default::default()
        };
        let (title, msg, _) = resolve(&p);
        assert_eq!(title, "Claude Code");
        assert_eq!(msg, "Claude notification");
    }

    #[test]
    fn resolve_stop() {
        let (title, msg, choices) = resolve(&payload_with_event("Stop"));
        assert_eq!(title, "Claude finished");
        assert!(msg.contains("waiting for your input"));
        assert!(choices.is_empty());
    }

    #[test]
    fn resolve_stop_failure_with_error_type() {
        let p = Payload {
            hook_event_name: Some("StopFailure".into()),
            error_type: Some("timeout".into()),
            ..Default::default()
        };
        let (title, msg, _) = resolve(&p);
        assert_eq!(title, "Claude error");
        assert!(msg.contains("timeout"));
    }

    #[test]
    fn resolve_stop_failure_no_error_type() {
        let (_, msg, _) = resolve(&payload_with_event("StopFailure"));
        assert!(msg.contains("unknown error"));
    }

    #[test]
    fn resolve_post_tool_use_with_tool_name() {
        let p = Payload {
            hook_event_name: Some("PostToolUse".into()),
            tool_name: Some("Bash".into()),
            ..Default::default()
        };
        let (title, msg, _) = resolve(&p);
        assert_eq!(title, "Tool completed");
        assert!(msg.contains("Bash"));
    }

    #[test]
    fn resolve_post_tool_use_no_tool_name() {
        let (_, msg, _) = resolve(&payload_with_event("PostToolUse"));
        assert!(msg.contains("Tool"));
    }

    #[test]
    fn resolve_post_tool_use_failure() {
        let p = Payload {
            hook_event_name: Some("PostToolUseFailure".into()),
            tool_name: Some("Edit".into()),
            ..Default::default()
        };
        let (title, msg, _) = resolve(&p);
        assert_eq!(title, "Tool failed");
        assert!(msg.contains("Edit"));
    }

    #[test]
    fn resolve_post_tool_use_failure_no_tool_name() {
        let (_, msg, _) = resolve(&payload_with_event("PostToolUseFailure"));
        assert!(msg.contains("Tool"));
    }

    #[test]
    fn resolve_subagent_stop_with_agent_type() {
        let p = Payload {
            hook_event_name: Some("SubagentStop".into()),
            agent_type: Some("researcher".into()),
            ..Default::default()
        };
        let (title, msg, _) = resolve(&p);
        assert_eq!(title, "Subagent done");
        assert!(msg.contains("researcher"));
    }

    #[test]
    fn resolve_subagent_stop_no_agent_type() {
        let (_, msg, _) = resolve(&payload_with_event("SubagentStop"));
        assert!(msg.contains("A subagent finished"));
    }

    #[test]
    fn resolve_teammate_idle_with_agent_type() {
        let p = Payload {
            hook_event_name: Some("TeammateIdle".into()),
            agent_type: Some("Alice".into()),
            ..Default::default()
        };
        let (title, msg, _) = resolve(&p);
        assert_eq!(title, "Teammate idle");
        assert!(msg.contains("Alice"));
    }

    #[test]
    fn resolve_teammate_idle_no_agent_type() {
        let (_, msg, _) = resolve(&payload_with_event("TeammateIdle"));
        assert!(msg.contains("Teammate"));
    }

    #[test]
    fn resolve_task_created_with_title() {
        let p = Payload {
            hook_event_name: Some("TaskCreated".into()),
            task_title: Some("Write tests".into()),
            ..Default::default()
        };
        let (title, msg, _) = resolve(&p);
        assert_eq!(title, "Task created");
        assert_eq!(msg, "Write tests");
    }

    #[test]
    fn resolve_task_created_no_title() {
        let (_, msg, _) = resolve(&payload_with_event("TaskCreated"));
        assert_eq!(msg, "New task");
    }

    #[test]
    fn resolve_task_completed() {
        let (title, msg, _) = resolve(&payload_with_event("TaskCompleted"));
        assert_eq!(title, "Task complete");
        assert!(msg.contains("completed"));
    }

    #[test]
    fn resolve_session_start() {
        let (title, msg, _) = resolve(&payload_with_event("SessionStart"));
        assert_eq!(title, "Session started");
        assert!(msg.contains("session has started"));
    }

    #[test]
    fn resolve_session_end_with_reason() {
        let p = Payload {
            hook_event_name: Some("SessionEnd".into()),
            reason: Some("timeout".into()),
            ..Default::default()
        };
        let (title, msg, _) = resolve(&p);
        assert_eq!(title, "Session ended");
        assert!(msg.contains("timeout"));
    }

    #[test]
    fn resolve_session_end_no_reason() {
        let (_, msg, _) = resolve(&payload_with_event("SessionEnd"));
        assert!(msg.contains("unknown"));
    }

    #[test]
    fn resolve_unknown_event_falls_back() {
        let (title, msg, _) = resolve(&payload_with_event("FutureEvent"));
        assert_eq!(title, "Claude Code");
        assert!(msg.contains("FutureEvent"));
    }

    #[test]
    fn resolve_no_event_name_treated_as_notification() {
        let p = Payload {
            notification_type: Some("idle_prompt".into()),
            message: Some("Hi".into()),
            ..Default::default()
        };
        let (title, _, _) = resolve(&p);
        assert_eq!(title, "Waiting for your input");
    }

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
}
