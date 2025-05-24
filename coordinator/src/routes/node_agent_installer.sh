#!/bin/sh

set -e

if [ -z "$1" ]; then
  echo "Usage: $0 <remote_url> [shuthost_node_agent install options...]"
  exit 1
fi

REMOTE_URL="$1"
# TODO: this aint right...
DEFAULT_PORT="${3:-5757}"
shift

BASE_URL="${REMOTE_URL}/download/node_agent"

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
        if getconf GNU_LIBC_VERSION >/dev/null 2>&1; then
            PLATFORM="linux"
        else
            PLATFORM="linux-musl"  # If not glibc, assume musl on Linux
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
OUTFILE="shuthost_node_agent"

echo "Downloading node_agent for $PLATFORM/$ARCH..."
curl -fL "$URL" -o "$OUTFILE"
chmod +x "$OUTFILE"

echo "Testing WOL packet reachability..."
# Start the test receiver in background
./"$OUTFILE" test-wol --port $(($DEFAULT_PORT + 1)) &
RECEIVER_PID=$!

# Give it time to start
sleep 1

# Test via coordinator API
TEST_RESULT=$(curl -s -X POST "$REMOTE_URL/api/test_wol")
kill $RECEIVER_PID

if echo "$TEST_RESULT" | grep -q "direct:true"; then
    echo "✓ Direct WOL packets working"
else
    echo "⚠️  Direct WoL packets failed - check firewall rules for UDP port 9"
fi

if echo "$TEST_RESULT" | grep -q "broadcast:true"; then
    echo "✓ Broadcast WoL packets working"
else
    echo "⚠️  Broadcast WoL packets failed - consider using direct WoL"
fi

elevate_privileges() {
    local cmd="$*"
    if command -v sudo >/dev/null 2>&1; then
        sudo $cmd
    elif command -v doas >/dev/null 2>&1; then
        doas $cmd
    else
        echo "Error: Neither sudo nor doas found. Please install sudo or doas."
        exit 1
    fi
}

echo "Running installer..."
elevate_privileges ./"$OUTFILE" install "$@"

echo "Cleaning up..."
rm -f "$OUTFILE"
