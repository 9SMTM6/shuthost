#!/bin/sh

# This script installs the shuthost host agent by determining the correct binary
# for the current platform and architecture from the coordinator,
# downloading it, and running the agent's installer with elevated privileges.

set -e

# TODO: consider a way to embed the install options. Main issue outside of it being annoying: Different defaults between OSs (mostly shutdown argument)
if [ -z "$1" ]; then
  echo "Usage: $0 <remote_url> [--arch <arch>] [--os <os>] [shuthost_host_agent install options...]"
  echo "  --arch <arch>   Override detected architecture (e.g. x86_64, aarch64)"
  echo "  --os <os>       Override detected OS/platform (e.g. linux, linux-musl, macos)"
  exit 1
fi

REMOTE_URL="$1"
shift

# Determine if we should accept self-signed certificates (for localhost/testing)
HOST=$(echo "$REMOTE_URL" | sed -e 's|^https*://||' -e 's|/.*$||' -e 's|:.*$||')
if [ "$HOST" = "localhost" ] || echo "$HOST" | grep -q '^127\.'; then
    CURL_OPTS="-k"
else
    CURL_OPTS=""
fi

DEFAULT_PORT="5757"
USER_ARCH=""
USER_OS=""
INSTALLER_ARGS=""

# Parse arguments for --arch and --os, and extract port
while [ $# -gt 0 ]; do
    case "$1" in
        --arch=*)
            USER_ARCH="${1#--arch=}"
            ;;
        --arch)
            shift
            USER_ARCH="$1"
            ;;
        --os=*)
            USER_OS="${1#--os=}"
            ;;
        --os)
            shift
            USER_OS="$1"
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
        *)
            # Escape any embedded double quotes
            ESCAPED_ARG=$(printf '%s' "$1" | sed 's/\"/\\\"/g')
            INSTALLER_ARGS="$INSTALLER_ARGS $ESCAPED_ARG"
            ;;
    esac
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

exit_on_glibc_error() {
    if [ "$PLATFORM" = "linux" ]; then
        if ! ./"$OUTFILE" --version >/dev/null 2>&1; then
            ERROR_OUTPUT=$(./"$OUTFILE" --version 2>&1)
            if echo "$ERROR_OUTPUT" | grep -q "version \`GLIBC_"; then
                echo "Detected glibc-related linker error: $ERROR_OUTPUT"
                echo "Recommendation: Use the 'linux-musl' version for better compatibility."
                echo "To override the platform, use the following command:"
                echo "curl -fsSL ${REMOTE_URL}/download/host_agent_installer.sh | sh -s ${REMOTE_URL} --os linux-musl $INSTALLER_ARGS"
                exit 1
            fi
        fi
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

# Detect architecture (allow override)
if [ -n "$USER_ARCH" ]; then
    ARCH="$USER_ARCH"
else
ARCH="$(uname -m)"
case "$ARCH" in
    x86_64) ARCH="x86_64" ;;
    aarch64 | arm64) ARCH="aarch64" ;;
    *)
        echo "Unsupported architecture: $ARCH"
        exit 1
        ;;
esac
fi

# Detect OS and MUSL (allow override)
if [ -n "$USER_OS" ]; then
    PLATFORM="$USER_OS"
else
OS="$(uname -s)"
case "$OS" in
    Linux)
        if getconf GNU_LIBC_VERSION >/dev/null 2>&1; then
            PLATFORM="linux"
        else
                PLATFORM="linux-musl"
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
fi

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

exit_on_glibc_error

test_wol_packet_reachability

# shellcheck disable=SC2090,SC2086
run_as_elevated ./$OUTFILE install $INSTALLER_ARGS

set +v

echo "Cleaning up..."
rm -f "$OUTFILE"
