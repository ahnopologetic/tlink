<p align="center">
  <img src="https://raw.githubusercontent.com/ahnopologetic/tlink/main/assets/readme-logo.png" alt="tlink logo" width="200">
</p>

<h3 align="center">tlink</h3>
<p align="center">Jump to any tmux session, window, or pane from a URL.</p>

---

```
open tmux://work/editor/0
```

`tlink` registers the `tmux://` URI scheme and routes clicks to the exact pane — flashing the border and showing a status-bar toast on arrival. It also ships notification addons that ping you when an AI coding agent finishes a task.

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/ahnopologetic/tlink/main/install.sh | sh
```

Detects your OS and architecture, installs to `~/.local/bin`, and adds it to your PATH. No sudo required.

**From source**
```bash
cargo install --git https://github.com/ahnopologetic/tlink
```

## Setup (macOS)

```bash
tlink setup
```

Runs a TUI wizard that picks your terminal emulator, compiles a minimal Swift handler app, and registers the `tmux://` scheme with macOS. Takes ~30 seconds, run once.

> Linux: URI scheme registration is macOS-only. `tlink open` (pane navigation) and the notification addon work on Linux without setup.

## Usage

```bash
open tmux://mysession
open tmux://mysession/editor
open tmux://mysession/editor/1
```

## Commands

| Command | Description |
|---|---|
| `tlink setup` | Register the `tmux://` URI scheme (macOS) |
| `tlink open <uri>` | Navigate to a tmux pane |
| `tlink install claude-notification` | Install the Claude Code notification addon |
| `tlink install codex-notification` | Install the Codex CLI notification addon |
| `tlink install gemini-notification` | Install the Gemini CLI notification addon |
| `tlink install pi-notification` | Install the Pi agent notification addon |
| `tlink install --interactive` | Interactive add-on selector (multi-select) |
| `tlink status` | Show registration state and active sessions |
| `tlink doctor` | Run diagnostic checks |
| `tlink restart` | Re-register the URI handler |

## Addons

### Interactive install

Use `tlink install -i` or `tlink install --interactive` to open a TUI that lists all available add-ons with checkboxes. Select multiple add-ons and install them all at once.

```bash
tlink install -i
```

### claude-notification

Desktop notifications from Claude Code hooks — with interactive Allow/Deny buttons for permission prompts and choice buttons for questions.

```bash
tlink install claude-notification
```

→ [Full docs](docs/claude-notification.md)

### codex-notification

Desktop notifications from Codex CLI hooks.

```bash
tlink install codex-notification
```

→ [Full docs](docs/codex-notification.md)

### gemini-notification

Desktop notifications from Gemini CLI hooks.

```bash
tlink install gemini-notification
```

→ [Full docs](docs/gemini-notification.md)

### pi-notification

Desktop notifications from Pi agent events.

```bash
tlink install pi-notification
```

→ [Full docs](docs/pi-notification.md)

## Platform support

| Feature | macOS | Linux |
|---|---|---|
| `tmux://` URI scheme | ✓ | — |
| Pane navigation (`tlink open`) | ✓ | ✓ |
| Status-bar toast | ✓ | ✓ |
| claude-notification addon | ✓ (terminal-notifier) | ✓ (dunstify / notify-send) |
| codex-notification addon | ✓ (terminal-notifier) | ✓ (dunstify / notify-send) |
| gemini-notification addon | ✓ (terminal-notifier) | ✓ (dunstify / notify-send) |
| pi-notification addon | ✓ (terminal-notifier) | ✓ (dunstify / notify-send) |

## License

MIT
