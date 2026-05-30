# pi-notification

Desktop notifications from [Pi](https://pi.ai) agent events. Get pinged when Pi finishes a task, a turn completes, or a tool execution finishes — without watching the terminal.

## Install

```bash
tlink install pi-notification
```

The interactive wizard lets you select which Pi events trigger notifications and installs a Pi extension at `~/.pi/agent/extensions/pi-notification.ts`.

## How it works

Pi fires extension events (`agent_end`, `session_start`, `turn_end`, etc.) to a registered TypeScript extension. That extension captures the current tmux session, window, and pane, then pipes a JSON payload to `tlink notify` via stdin, which fires a desktop notification via the configured backend.

When you click the notification (on supported backends), it navigates your terminal to the exact tmux pane where Pi was running.

## Hook events

| Event | Description |
|---|---|
| `agent_end` | Pi finished responding and is waiting for input |
| `turn_end` | An LLM turn completed (good for long tasks) |
| `session_start` | A Pi session has started |
| `session_shutdown` | A Pi session has ended |
| `tool_execution_end` | A tool execution completed |

## Uninstall

```bash
tlink delete pi-notification
```

Removes the extension file from `~/.pi/agent/extensions/pi-notification.ts`.
