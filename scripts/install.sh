#!/bin/sh
set -eu

REPO="ulughbeck/knack"
BIN="knack"

usage() {
  cat <<USAGE
Usage: install.sh [version]

Installs the latest or specified version of ${BIN} from GitHub Releases.

Arguments:
  version   Git tag like v0.0.1 (optional)

Environment:
  INSTALL_DIR   Install destination (default: /usr/local/bin if writable, else ~/.local/bin)
  VERSION       Same as version argument
USAGE
}

if [ "${1-}" = "-h" ] || [ "${1-}" = "--help" ]; then
  usage
  exit 0
fi

VERSION="${VERSION-}"
if [ -n "${1-}" ]; then
  VERSION="$1"
fi

if [ -z "$VERSION" ]; then
  VERSION="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | awk -F '"' '/"tag_name":/ { print $4; exit }')"
fi

if [ -z "$VERSION" ]; then
  echo "Could not determine latest version." >&2
  exit 1
fi

OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Darwin) PLATFORM="macos" ;;
  Linux) PLATFORM="linux" ;;
  *)
    echo "Unsupported OS: $OS" >&2
    exit 1
    ;;
esac

case "$ARCH" in
  arm64|aarch64) ARCH="arm64" ;;
  x86_64) ARCH="x86_64" ;;
  *)
    echo "Unsupported architecture: $ARCH" >&2
    exit 1
    ;;
esac

ASSET="knack-${PLATFORM}-${ARCH}.tar.gz"

TMP_DIR="$(mktemp -d)"
cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT INT TERM

URL="https://github.com/${REPO}/releases/download/${VERSION}/${ASSET}"

curl -fL -o "$TMP_DIR/$ASSET" "$URL"

if ! tar -xzf "$TMP_DIR/$ASSET" -C "$TMP_DIR"; then
  echo "Failed to extract $ASSET" >&2
  exit 1
fi

INSTALL_DIR="${INSTALL_DIR-}"
if [ -z "$INSTALL_DIR" ]; then
  if [ -w "/usr/local/bin" ]; then
    INSTALL_DIR="/usr/local/bin"
  else
    INSTALL_DIR="$HOME/.local/bin"
  fi
fi

mkdir -p "$INSTALL_DIR"
cp "$TMP_DIR/$BIN" "$INSTALL_DIR/$BIN"
chmod 755 "$INSTALL_DIR/$BIN"

if ! printf '%s' "$PATH" | tr ':' '\n' | grep -qx "$INSTALL_DIR"; then
  echo "Added $BIN to $INSTALL_DIR, but that path is not on your PATH."
  echo "Add this to your shell profile:"
  echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
fi

echo "Installed $BIN $VERSION to $INSTALL_DIR/$BIN"
