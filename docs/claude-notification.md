# claude-notification

Desktop notifications from [Claude Code](https://claude.ai/code) hooks. Get pinged when Claude finishes a task, needs permission, or asks you a question — without watching the terminal.

## Install

```bash
tlink install claude-notification
```

The interactive wizard picks a notification method and registers the hook in `~/.claude/settings.json`.

## How it works

Claude Code fires hook events (Stop, Notification, etc.) to a script. That script calls `tlink notify`, which reads the JSON payload from stdin, resolves it to a title and message, and fires a desktop notification via the configured backend.

For interactive events, clicking a button injects keystrokes into the originating tmux pane via `tmux send-keys` — so you can approve a permission prompt from the notification without switching windows.

## Notification backends

| Backend | Platform | Notes |
|---|---|---|
| `alerter` | macOS 12+ | Click-to-navigate, interactive buttons. Recommended. |
| `terminal-notifier` | macOS | Click navigation broken on macOS 12+. |
| `osascript` | macOS | Built-in fallback, no click callback. |
| `dunstify` | Linux | Click-to-navigate via dunst daemon. |
| `notify-send` | Linux | Basic, no click action. |

Switch backends anytime:

```bash
tlink install claude-notification   # re-run wizard to change method
```

## Hook events

| Event | Trigger |
|---|---|
| `Stop` | Claude finished responding |
| `StopFailure` | Turn ended with an API error |
| `Notification / idle_prompt` | Claude is waiting for your input |
| `Notification / permission_prompt` | Claude needs approval to run a tool |
| `Notification / elicitation_dialog` | An MCP server is asking you a question |
| `PostToolUse` | A tool call completed |
| `SubagentStop` | A subagent finished |
| `SessionStart` / `SessionEnd` | Session lifecycle |

## Interactive notifications

### Permission prompts

When Claude asks to run a tool, the notification shows **Allow** and **Deny** buttons.

- **Allow** → sends `y` + Enter to the pane (approves once)
- **Deny** → sends `n` + Enter to the pane (denies)
- **Body click** → navigates to the pane

### Elicitation (MCP questions)

When an MCP server asks a question with choices, the notification shows those choices as buttons. Clicking one types the choice text into the pane and presses Enter.

> Responses are injected via `tmux send-keys`. If you dismiss the prompt in the terminal before clicking the notification, the keystrokes will land in whatever is focused at that moment.

## Uninstall

```bash
tlink delete claude-notification
```

Removes the hook script and deregisters all tlink entries from `~/.claude/settings.json`.
