# codex-notification

Desktop notifications from [Codex CLI](https://codex.cli). Get pinged when Codex finishes a task — without watching the terminal.

## Install

```bash
tlink install codex-notification
```

The interactive wizard picks a notification method and registers the `notify` config in `~/.codex/config.toml`.

## How it works

Codex CLI runs a notification command when a turn ends (`turn-ended` event). Our hook script captures the current tmux session, window, and pane context, then calls `tlink notify` which fires a desktop notification via the configured backend.

When you click the notification (on supported backends), it navigates your terminal to the exact tmux pane where Codex was running.

## Notification backends

| Backend | Platform | Notes |
|---|---|---|
| `terminal-notifier` | macOS | Click-to-navigate. Recommended. |
| `osascript` | macOS | Built-in fallback, no click callback. |
| `dunstify` | Linux | Click-to-navigate via dunst daemon. |
| `notify-send` | Linux | Basic, no click action. |

Switch backends anytime:

```bash
tlink install codex-notification   # re-run wizard to change method
```

## Configuration

The add-on sets the `notify` key in `~/.codex/config.toml`:

```toml
notify = ["~/.config/tlink/hooks/codex-notification.sh", "turn-ended"]
```

## Uninstall

```bash
tlink delete codex-notification
```

Removes the hook script and clears the `notify` entry from `~/.codex/config.toml`.
