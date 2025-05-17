#!/bin/sh

set -e

if [ -z "$1" ]; then
  echo "Usage: $0 <remote_url> [shuthost_agent install options...]"
  exit 1
fi

REMOTE_URL="$1"
shift

BASE_URL="${REMOTE_URL}download/agent"

# Detect architecture
ARCH="$(uname -m)"
case "$ARCH" in
    x86_64) ARCH="x86_64" ;;
    aarch64 | arm64) ARCH="aarch64" ;;
    *)
        echo "Unsupported architecture: $ARCH"
        exit 1
        ;;
esac

# Detect OS and MUSL
OS="$(uname -s)"
case "$OS" in
    Linux)
        if ldd --version 2>&1 | grep -q musl; then
            PLATFORM="linux-musl"
        else
            PLATFORM="linux"
        fi
        ;;
    Darwin)
        PLATFORM="macos"
        ;;
    *)
        echo "Unsupported OS: $OS"
        exit 1
        ;;
esac

URL="$BASE_URL/$PLATFORM/$ARCH"
OUTFILE="shuthost_agent"

echo "Downloading agent for $PLATFORM/$ARCH..."
curl -fL "$URL" -o "$OUTFILE"
chmod +x "$OUTFILE"

echo "Running installer..."
sudo ./"$OUTFILE" install "$@"

echo "Cleaning up..."
rm -f "$OUTFILE"
