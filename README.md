<p align="center">
  <img src="https://raw.githubusercontent.com/ahnopologetic/tlink/main/assets/readme-logo.png" alt="tlink logo" width="200">
</p>

<h3 align="center">tlink</h3>
<p align="center">Jump to any tmux session, window, or pane from a URL.</p>

---

```
open tmux://work/editor/0
```

`tlink` registers the `tmux://` URI scheme and routes clicks to the exact pane — flashing the border and showing a status-bar toast on arrival. It also ships a [Claude Code notification addon](docs/claude-notification.md) that pings you when Claude finishes a task.

## Install

**macOS / Linux — binary**

```bash
VERSION=v0.1.2
# macOS
ARCH=$(uname -m); [ "$ARCH" = "arm64" ] && ARCH="aarch64"
curl -fsSL "https://github.com/ahnopologetic/tlink/releases/download/${VERSION}/tlink-${ARCH}-apple-darwin" \
  -o /usr/local/bin/tlink && chmod +x /usr/local/bin/tlink

# Linux (x86_64, glibc)
curl -fsSL "https://github.com/ahnopologetic/tlink/releases/download/${VERSION}/tlink-x86_64-unknown-linux-gnu" \
  -o /usr/local/bin/tlink && chmod +x /usr/local/bin/tlink
```

See [Releases](https://github.com/ahnopologetic/tlink/releases/latest) for all targets (ARM64, musl, ARMv7).

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
| `tlink status` | Show registration state and active sessions |
| `tlink doctor` | Run diagnostic checks |
| `tlink restart` | Re-register the URI handler |

## Addons

### claude-notification

Desktop notifications from Claude Code hooks — with interactive Allow/Deny buttons for permission prompts and choice buttons for questions.

```bash
tlink install claude-notification
```

→ [Full docs](docs/claude-notification.md)

## Platform support

| Feature | macOS | Linux |
|---|---|---|
| `tmux://` URI scheme | ✓ | — |
| Pane navigation (`tlink open`) | ✓ | ✓ |
| Status-bar toast | ✓ | ✓ |
| claude-notification addon | ✓ (alerter) | ✓ (dunstify / notify-send) |

## License

MIT
