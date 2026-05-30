# gemini-notification

Desktop notifications from [Gemini CLI](https://geminicli.com). Get pinged when Gemini finishes a task, starts a session, or triggers a hook event — without watching the terminal.

## Install

```bash
tlink install gemini-notification
```

The interactive wizard picks a notification method and registers hooks in `~/.gemini/settings.json`.

## How it works

Gemini CLI fires hook events (`AfterAgent`, `SessionStart`, etc.) to a registered command script. That script captures the current tmux session, window, and pane, parses the hook payload from stdin, and calls `tlink notify` to fire a desktop notification via the configured backend.

When you click the notification (on supported backends), it navigates your terminal to the exact tmux pane where Gemini was running.

## Notification backends

| Backend | Platform | Notes |
|---|---|---|
| `terminal-notifier` | macOS | Click-to-navigate. Recommended. |
| `osascript` | macOS | Built-in fallback, no click callback. |
| `dunstify` | Linux | Click-to-navigate via dunst daemon. |
| `notify-send` | Linux | Basic, no click action. |

Switch backends anytime:

```bash
tlink install gemini-notification   # re-run wizard to change method
```

## Hook events

| Event | Trigger |
|---|---|
| `AfterAgent` | Gemini CLI finished responding and is waiting for input |
| `SessionStart` | A Gemini CLI session has started |
| `SessionEnd` | A Gemini CLI session has ended |
| `TaskCreated` | A new task was created |
| `TaskCompleted` | A task was completed |
| `BeforeTool` | Before a tool execution begins |
| `AfterTool` | After a tool execution completes |

## Configuration

The add-on registers hook entries in `~/.gemini/settings.json`:

```json
{
  "hooks": {
    "AfterAgent": [
      {
        "matcher": "*",
        "hooks": [{ "type": "command", "command": "~/.config/tlink/hooks/gemini-notification.sh" }]
      }
    ]
  }
}
```

## Uninstall

```bash
tlink delete gemini-notification
```

Removes the hook script and deregisters all tlink entries from `~/.gemini/settings.json`.
