<p align="center">
  <img src="https://raw.githubusercontent.com/ahnopologetic/tlink/main/assets/readme-logo.png" alt="tlink logo" width="200">
</p>

<h3 align="center">tlink</h3>
<p align="center">Jump to any tmux session, window, or pane from a URL.</p>

---

```
open tmux://<session>/<window>/<pane>
```

`tlink` registers the `tmux://` URI scheme and routes clicks to the exact pane — flashing the border and showing a status-bar toast on arrival. It also ships notification addons that ping you when an AI coding agent finishes a task.

> **Ghostty note:** Ghostty doesn't expose an API to focus a specific tab or open a new window with a command without triggering a macOS security dialog. When you click a `tmux://` link, it works the same as other terminals (switches the session in place), but if you're in a different tab you'll need to switch to the tmux tab manually.

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

### Click a link (from anywhere)

Once `tlink setup` is done, any `tmux://` link is clickable — in Slack, in a browser, in a chat app, wherever. macOS routes it through TmuxLink.app and into `tlink open`.

```bash
# Click these from anywhere — they open in your terminal
tmux://mysession
tmux://mysession/work
tmux://mysession/work/1
```

Links target three levels of specificity:

| Target | Example | What happens |
|--------|---------|-------------|
| Session | `tmux://work` | Switches to the `work` session |
| Session + window | `tmux://work/editor` | Switches to window `editor` in `work` |
| Session + window + pane | `tmux://work/editor/0` | Switches to pane 0 in `editor` |

If you're already in tmux, the current pane switches to the target and flashes green. A status-bar toast confirms the jump:

```
[tlink] tlink → work:editor.0
```

If no tmux client is attached (you clicked the link from outside a terminal), `tlink` falls back to asking your terminal to open a new window running `tmux attach-session -t <session>`. This works on Terminal.app, iTerm2, Kitty, and WezTerm.

### Run from a terminal

```bash
# Direct open — same as clicking a link
tlink open "tmux://mysession"

# Or use macOS open command
tlink open "tmux://mysession/work/1"
```

### Notification addons

When an AI coding agent (Claude Code, Codex CLI, Gemini CLI, Pi) finishes a task, tlink can ping you with a desktop notification:

```bash
tlink install claude-notification
tlink install pi-notification
tlink install --interactive
```

The notification includes a clickable deeplink back to the session, window, and pane where the agent was running. See [Addons](#addons) below.

### Telemetry

```bash
tlink telemetry status      # Check current setting
tlink telemetry enable      # Opt in (anonymous usage data)
tlink telemetry disable     # Opt out
```

Activity events are written locally to `~/.local/share/tlink/telemetry/events.jsonl`. If a Sentry DSN is configured (set `TLINK_SENTRY_DSN` at build time), errors and activity are sent to Sentry for diagnostics.

### Diagnostics

```bash
tlink status     # Registration state + active tmux sessions
tlink doctor     # Run all diagnostic checks
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
| `tlink delete <addon>` | Remove a notification addon |
| `tlink telemetry status` | Show telemetry setting |
| `tlink telemetry enable` | Opt into anonymous usage data |
| `tlink telemetry disable` | Opt out |
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

## Uninstall

```bash
curl -fsSL https://raw.githubusercontent.com/ahnopologetic/tlink/main/uninstall.sh | sh
```

Removes:

| What | Location |
|---|---|
| Binary | `~/.local/bin/tlink` |
| Config | `~/.config/tlink/` (includes hook scripts) |
| Telemetry data | `~/.local/share/tlink/` (events, machine-id) |
| URI handler | `~/Applications/TmuxLink.app` |
| URI scheme registration | Unregistered via `lsregister` (macOS) |

If you installed via `cargo install`, also run:

```bash
cargo uninstall tlink
```

To restore your PATH, remove `export PATH="$HOME/.local/bin:$PATH"` from your shell config (`.zshrc`, `.bashrc`, etc.).

## License

MIT
