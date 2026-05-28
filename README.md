# tlink

`tmux://` deeplink CLI for macOS — jump to any tmux session, window, or pane from a URL.

```
open tmux://work/editor/0
```

Clicking a `tmux://` link switches your terminal to the exact session, window, and pane, flashes the border, and shows a brief status-bar toast.

---

## Prerequisites

- macOS 12+
- [tmux](https://github.com/tmux/tmux)
- Xcode Command Line Tools (for `swiftc`, used once during setup)

```bash
xcode-select --install
brew install tmux   # if not already installed
```

---

## Install

### Download binary (recommended)

```bash
# Detect architecture and install
ARCH=$(uname -m)
[ "$ARCH" = "arm64" ] && ARCH="aarch64"
curl -fsSL "https://github.com/ahnopologetic/tlink/releases/latest/download/tlink-${ARCH}-apple-darwin" \
  -o /usr/local/bin/tlink
chmod +x /usr/local/bin/tlink
```

Or download manually from [Releases](https://github.com/ahnopologetic/tlink/releases/latest):

| Architecture | File |
|---|---|
| Apple Silicon (M1/M2/M3) | `tlink-aarch64-apple-darwin` |
| Intel | `tlink-x86_64-apple-darwin` |

### Build from source

```bash
git clone https://github.com/ahnopologetic/tlink
cd tlink
cargo install --path .
```

Requires Rust stable (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`).

---

## Setup

Run the interactive TUI wizard once after installing:

```bash
tlink setup
```

The wizard will:
1. Detect installed terminal emulators
2. Let you pick yours (iTerm2, Ghostty, Kitty, WezTerm, Terminal.app)
3. Compile a minimal Swift handler app and register the `tmux://` URI scheme
4. Verify everything works

Setup takes about 30 seconds. You only need to run it once (or after a macOS update breaks the handler — use `tlink restart` then).

---

## Usage

```bash
# Switch to a session
open tmux://mysession

# Switch to a specific window (name or index)
open tmux://mysession/editor
open tmux://mysession/2

# Switch to a specific pane
open tmux://mysession/editor/1
open tmux://mysession/2/0
```

On activation, tlink:
- Switches your terminal client to the target
- Flashes a bright border on the destination pane for 1.5 s
- Shows `tlink → session:window.pane` in the status bar for 2 s

---

## Commands

| Command | Description |
|---|---|
| `tlink setup` | Interactive TUI wizard — first-time registration |
| `tlink open <uri>` | Handle a `tmux://` URI (also called by the OS) |
| `tlink status` | Show registration state, configured terminal, active sessions |
| `tlink restart` | Re-register the URI handler (after macOS updates, etc.) |
| `tlink doctor` | Run 6 diagnostic checks, exits 1 on failure |

---

## How it works

1. `tlink setup` compiles a minimal Swift app (`~/Applications/TmuxLink.app`) that receives macOS Apple Events for the `tmux://` scheme and calls `tlink open <url>`.
2. The app is registered with Launch Services via `lsregister`.
3. When any app calls `open tmux://…`, macOS routes it to `TmuxLink.app`, which calls `tlink open`, which runs `tmux switch-client -t session:window.pane`.

---

## Troubleshooting

```bash
tlink doctor      # check what's broken
tlink restart     # re-register after macOS update
tlink status      # inspect current state
```

If `tlink doctor` shows failures after a macOS update, `tlink restart` usually fixes it. If `swiftc` is missing, run `xcode-select --install`.

---

## License

MIT
