#!/bin/sh

set -e

# TODO: consider a way to embed the install options. Main issue outside of it being annoying: Different defaults between OSs (mostly shutdown argument)
if [ -z "$1" ]; then
  echo "Usage: $0 <remote_url> [shuthost_node_agent install options...]"
  exit 1
fi

REMOTE_URL="$1"
shift
# Extract port from installation arguments arguments while preserving them
DEFAULT_PORT="5757"
for arg in "$@"; do
    if [ "${arg#--port=}" != "$arg" ]; then
        DEFAULT_PORT="${arg#--port=}"
    elif [ "$prev_arg" = "--port" ]; then
        DEFAULT_PORT="$arg"
    fi
    prev_arg="$arg"
done

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

OUTFILE="shuthost_node_agent"

echo "Downloading node_agent for $PLATFORM/$ARCH..."
curl -fL "${REMOTE_URL}/download/node_agent/$PLATFORM/$ARCH" -o "$OUTFILE"
chmod +x "$OUTFILE"

WOL_TEST_PORT=$((DEFAULT_PORT + 1))

echo "Testing WOL packet reachability..."
# Start the test receiver in background
./"$OUTFILE" test-wol --port $WOL_TEST_PORT &
RECEIVER_PID=$!

# Give it time to start
sleep 1

# Test via coordinator API
TEST_RESULT=$(curl -s -X POST "$REMOTE_URL/api/m2m/test_wol?port=$WOL_TEST_PORT")
# kill the agent test process, if its still running.
kill $RECEIVER_PID || true

if echo "$TEST_RESULT" | grep -q "\"direct\":true"; then
    echo "✓ Direct WOL packets working"
else
    echo "⚠️  Direct WoL packets failed - check firewall rules for UDP port 9"
fi

if echo "$TEST_RESULT" | grep -q "\"broadcast\":true"; then
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
if [ "$(id -u)" -eq 0 ]; then
    ./"$OUTFILE" install "$@"
else
    elevate_privileges ./"$OUTFILE" install "$@"
fi

echo "Cleaning up..."
rm -f "$OUTFILE"
