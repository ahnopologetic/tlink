#!/bin/sh
set -e

echo "Uninstalling tlink..."
echo ""

# Binary
BIN_DIR="$HOME/.local/bin"
BIN="$BIN_DIR/tlink"
if [ -f "$BIN" ]; then
  rm -v "$BIN"
  echo "  Removed $BIN"
  # Remove BIN_DIR if it's empty
  rmdir "$BIN_DIR" 2>/dev/null || true
else
  echo "  No binary found at $BIN"
fi

# Config (includes hook scripts, config.toml)
CONFIG_DIR="$HOME/.config/tlink"
if [ -d "$CONFIG_DIR" ]; then
  rm -rv "$CONFIG_DIR"
  echo "  Removed config"
else
  echo "  No config found"
fi

# Telemetry data (events, machine-id)
DATA_DIR="$HOME/.local/share/tlink"
if [ -d "$DATA_DIR" ]; then
  rm -rv "$DATA_DIR"
  echo "  Removed telemetry data"
else
  echo "  No telemetry data found"
fi

# TmuxLink handler app
TMUXLINK_APP="$HOME/Applications/TmuxLink.app"
if [ -d "$TMUXLINK_APP" ]; then
  rm -rv "$TMUXLINK_APP"
  echo "  Removed TmuxLink.app"

  # Unregister the tmux:// scheme on macOS
  if [ "$(uname -s)" = "Darwin" ]; then
    LSREGISTER="/System/Library/Frameworks/CoreServices.framework/Versions/A/Frameworks/LaunchServices.framework/Versions/A/Support/lsregister"
    if [ -x "$LSREGISTER" ]; then
      "$LSREGISTER" -u "$TMUXLINK_APP" 2>/dev/null || true
      echo "  Unregistered tmux:// URI scheme"
    fi
  fi
else
  echo "  No TmuxLink.app found"
fi

echo ""
echo "tlink uninstalled."
echo "To also remove the tlink source (if installed via cargo):  cargo uninstall tlink"
echo "To restore PATH: remove 'export PATH=\"\$HOME/.local/bin:\$PATH\"' from your shell config."