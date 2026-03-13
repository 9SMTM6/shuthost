#!/bin/sh

# This script installs the shuthost host agent by determining the correct binary
# for the current platform and architecture from the coordinator,
# downloading it, and running the agent's installer with elevated privileges.

set -e

# Parse options
HELP=false
REMOTE_URL=""
DEFAULT_PORT="9090"
INSTALLER_ARGS=""

while [ $# -gt 0 ]; do
    case "$1" in
        -h|--help)
            HELP=true
            break
            ;;
        --)
            shift
            break
            ;;
        --port=*)
            DEFAULT_PORT="${1#--port=}"
            INSTALLER_ARGS="$INSTALLER_ARGS $1"
            ;;
        --port)
            shift
            DEFAULT_PORT="$1"
            INSTALLER_ARGS="$INSTALLER_ARGS --port $1"
            ;;
        -*)
            echo "Unknown option: $1" >&2
            exit 1
            ;;
        *)
            if [ -z "$REMOTE_URL" ]; then
                REMOTE_URL="$1"
            else
                echo "Unexpected argument: $1" >&2
                exit 1
            fi
            ;;
    esac
    shift
done

if $HELP; then
    echo "Usage: $0 <remote_url> [--port PORT] [-- <install_args>]"
    echo "Install ShutHost host agent from coordinator."
    echo ""
    echo "Arguments:"
    echo "  remote_url     URL of the coordinator"
    echo "  --port PORT    Port for WoL testing (default: 9090)"
    echo "  -- <args>      Additional arguments for the host agent install command"
    exit 0
fi

if [ -z "$REMOTE_URL" ]; then
    echo "Error: remote_url is required" >&2
    exit 1
fi

# Determine if we should accept self-signed certificates (for localhost/testing)
HOST=$(echo "$REMOTE_URL" | sed -e 's|^https*://||' -e 's|/.*$||' -e 's|:.*$||')
if [ "$HOST" = "localhost" ] || echo "$HOST" | grep -q '^127\.'; then
    CURL_OPTS="-k"
else
    CURL_OPTS=""
fi

# Collect remaining as binary args
BINARY_ARGS=""
while [ $# -gt 0 ]; do
    # Escape any embedded double quotes
    ESCAPED_ARG=$(printf '%s' "$1" | sed 's/\"/\\\"/g')
    if printf '%s' "$ESCAPED_ARG" | grep -q '[[:space:]]'; then
        BINARY_ARGS="$BINARY_ARGS \"$ESCAPED_ARG\""
    else
        BINARY_ARGS="$BINARY_ARGS $ESCAPED_ARG"
    fi
    shift
done

elevate_privileges() {
    cmd="$*"
    if command -v sudo >/dev/null 2>&1; then
        # shellcheck disable=SC2086
        sudo $cmd
    elif command -v doas >/dev/null 2>&1; then
        # shellcheck disable=SC2086
        doas $cmd
    else
        echo "Error: Neither sudo nor doas found. Please install sudo or doas."
        exit 1
    fi
}

run_as_elevated() {
    if [ "$(id -u)" -eq 0 ]; then
        sh -c "$*"
    else
        elevate_privileges "$*"
    fi
}

test_wol_packet_reachability() {
    WOL_TEST_PORT=$((DEFAULT_PORT + 1))

    # Start the test receiver in background
    ./"$OUTFILE" test-wol --port $WOL_TEST_PORT &
    RECEIVER_PID=$!

    # Give it time to start
    sleep 1

    # Test via coordinator API
    TEST_RESULT=$(curl $CURL_OPTS -s -X POST "$REMOTE_URL/api/m2m/test_wol?port=$WOL_TEST_PORT" 2>/dev/null || echo "")
    # kill the agent test process, if its still running.
    kill $RECEIVER_PID || true

    if echo "$TEST_RESULT" | grep -q "\"broadcast\":true"; then
        echo "✓ Broadcast WoL packets working"
    else
        echo "⚠️  Broadcast WoL packets failed - check firewall rules for UDP port 9"
    fi
}

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

# Detect OS
OS="$(uname -s)"
case "$OS" in
    Linux)
        PLATFORM="linux-musl"
        ;;
    Darwin)
        PLATFORM="macos"
        ;;
    *)
        echo "Unsupported OS: $OS"
        exit 1
        ;;
esac

OUTFILE="shuthost_host_agent"

echo "Downloading host_agent for $PLATFORM/$ARCH..."

################## Boring setup complete ------------- Interesting stuff is starting here

set -v
echo "$REMOTE_URL"

echo "$ARCH"

echo "$PLATFORM"

echo "$INSTALLER_ARGS"


curl --compressed -fL $CURL_OPTS "${REMOTE_URL}/download/host_agent/$PLATFORM/$ARCH" -o "$OUTFILE"
chmod +x "$OUTFILE"

test_wol_packet_reachability

# shellcheck disable=SC2090,SC2086
run_as_elevated ./$OUTFILE install $BINARY_ARGS

set +v

echo "Cleaning up..."
rm -f "$OUTFILE"
