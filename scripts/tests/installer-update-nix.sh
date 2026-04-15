#!/bin/sh
# CI test for the coordinator installer update flow.
# This installs an older release via the enduser installer, starts the local coordinator,
# and then updates the agent through the coordinator installer.

set -eu

. ./scripts/helpers.sh

TARGET_TAG="1.6.4"
ENDUSER_INSTALLER="./scripts/enduser_installers/host_agent.sh"
COORDINATOR_INSTALLER="./scripts/coordinator_installers/host_agent.sh"
COORDINATOR_BINARY=""

if [ $# -ge 1 ]; then
    COORDINATOR_BINARY="$1"
fi

if [ -z "$COORDINATOR_BINARY" ]; then
    if [ -x ./target/release/shuthost_coordinator ]; then
        COORDINATOR_BINARY=./target/release/shuthost_coordinator
    elif [ -x ./target/debug/shuthost_coordinator ]; then
        COORDINATOR_BINARY=./target/debug/shuthost_coordinator
    else
        echo "Error: shuthost_coordinator binary not found in target/release or target/debug." >&2
        exit 1
    fi
fi

if [ ! -x "$COORDINATOR_BINARY" ]; then
    echo "Error: specified shuthost_coordinator binary is not executable: $COORDINATOR_BINARY" >&2
    exit 1
fi

find_free_port() {
    python3 - <<'PY'
import socket
with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
    s.bind(('127.0.0.1', 0))
    print(s.getsockname()[1])
PY
}

find_free_udp_port() {
    python3 - <<'PY'
import socket
with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as s:
    s.bind(('127.0.0.1', 0))
    print(s.getsockname()[1])
PY
}

COORDINATOR_PORT=$(find_free_port)
COORDINATOR_BROADCAST_PORT=$(find_free_udp_port)
COORDINATOR_URL="http://127.0.0.1:$COORDINATOR_PORT"

# Use a temporary directory for all outputs
TMPDIR=$(mktemp -d /tmp/shuthost_coordinator_update.XXXXXX)
trap 'if [ -n "${COORDINATOR_PID-}" ]; then kill "$COORDINATOR_PID" 2>/dev/null || true; fi; rm -rf "$TMPDIR"' EXIT

printf 'Starting coordinator installer update test\n'
printf 'Installing old release %s via enduser installer\n' "$TARGET_TAG"

export CI_MODE=1
sh "$ENDUSER_INSTALLER" -t "$TARGET_TAG"

printf 'Waiting for old release agent to become ready...\n'
wait_for_agent_ready

printf 'Generating direct control script for agent status check (old release)\n'
run_as_elevated shuthost_host_agent generate-direct-control -o "$TMPDIR/shuthost_direct_control_test"

printf 'Checking agent status using direct control script (old release)\n'
status_output=$("$TMPDIR/shuthost_direct_control_test" status)
echo "Agent status response: $status_output"
if echo "$status_output" | grep -q '^OK: status'; then
    if echo "$status_output" | grep -q 'agent_version='; then
        printf 'Agent version info present (unexpected for v1.6.4)!\n'
        exit 1
    else
        printf 'Agent status OK (old release, no version info as expected)\n'
    fi
else
    printf 'Agent status check failed (old release)\n'
    exit 1
fi

CONFIG_FILE="$TMPDIR/coordinator_config.toml"
cat > "$CONFIG_FILE" << EOF
[server]
port = $COORDINATOR_PORT
bind = "127.0.0.1"
broadcast_port = $COORDINATOR_BROADCAST_PORT

[db]
path = "$TMPDIR/shuthost.db"
enable = true

[hosts]

[clients]
EOF

printf 'Starting local coordinator directly from %s\n' "$COORDINATOR_BINARY"
"$COORDINATOR_BINARY" control-service --config "$CONFIG_FILE" > "$TMPDIR/coordinator.log" 2>&1 &
COORDINATOR_PID=$!

printf 'Waiting for local coordinator to become ready...\n'
wait_for_coordinator_ready "$COORDINATOR_PORT"

printf 'Updating host agent through the coordinator installer\n'
sh "$COORDINATOR_INSTALLER" "$COORDINATOR_URL" --update

printf 'Waiting for host agent after coordinator update to become ready...\n'
wait_for_agent_ready

printf 'Checking agent status using direct control script (after update)\n'
status_output=$("$TMPDIR/shuthost_direct_control_test" status)
echo "Agent status response: $status_output"
if echo "$status_output" | grep -q '^OK: status'; then
    if echo "$status_output" | grep -q 'agent_version='; then
        printf 'Agent status OK (updated, version info present)\n'
    else
        printf 'Agent status OK (updated, but no version info found!)\n'
        exit 1
    fi
else
    printf 'Agent status check failed (after update)\n'
    exit 1
fi

printf 'Coordinator installer update test completed successfully!\n'
