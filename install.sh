#!/bin/sh
set -e

REPO="ahnopologetic/tlink"
BIN_DIR="$HOME/.local/bin"
BIN="$BIN_DIR/tlink"

# Detect OS and arch
OS=$(uname -s)
ARCH=$(uname -m)

case "$OS" in
  Darwin)
    case "$ARCH" in
      arm64)  TARGET="aarch64-apple-darwin" ;;
      x86_64) TARGET="x86_64-apple-darwin" ;;
      *) echo "Unsupported architecture: $ARCH" >&2; exit 1 ;;
    esac
    ;;
  Linux)
    case "$ARCH" in
      x86_64)  TARGET="x86_64-unknown-linux-gnu" ;;
      aarch64) TARGET="aarch64-unknown-linux-gnu" ;;
      armv7l)  TARGET="armv7-unknown-linux-gnueabihf" ;;
      *) echo "Unsupported architecture: $ARCH" >&2; exit 1 ;;
    esac
    ;;
  *) echo "Unsupported OS: $OS" >&2; exit 1 ;;
esac

URL="https://github.com/$REPO/releases/latest/download/tlink-$TARGET"

echo "Installing tlink for $TARGET..."

mkdir -p "$BIN_DIR"
curl -fsSL "$URL" -o "$BIN"
chmod +x "$BIN"

echo "Installed to $BIN"

# Add ~/.local/bin to PATH if not already there
add_to_path() {
  FILE="$1"
  if [ -f "$FILE" ] && ! grep -q '\.local/bin' "$FILE"; then
    printf '\nexport PATH="$HOME/.local/bin:$PATH"\n' >> "$FILE"
    echo "Added ~/.local/bin to PATH in $FILE"
  fi
}

case "$SHELL" in
  */zsh)  add_to_path "$HOME/.zshrc" ;;
  */bash) add_to_path "$HOME/.bashrc"; add_to_path "$HOME/.bash_profile" ;;
  *)      add_to_path "$HOME/.profile" ;;
esac

echo ""
echo "Done. Run: tlink --help"
echo "(Restart your shell or run: export PATH=\"\$HOME/.local/bin:\$PATH\")"
