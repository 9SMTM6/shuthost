#!/bin/sh
# CI script to test direct host_agent installation and direct_control script on systemd
# Usage: install-and-run-direct-control.sh [host_agent_binary] [user]
# Defaults: ./shuthost_host_agent, root

set -eu

. ./scripts/helpers.sh

HOST_AGENT_BINARY="${1:-./shuthost_host_agent}"

printf 'Testing direct host_agent installation:\n  Host agent binary: %s\n' "$HOST_AGENT_BINARY"

export RUST_BACKTRACE=1

set -v

run_as_elevated "$HOST_AGENT_BINARY install --shutdown-command=\"touch /tmp/shutdown_executed\""

wait_for_agent_ready

run_as_elevated pgrep -af shuthost_host_agent || { printf 'Host agent service is not running\n' ; exit 1; }

run_as_elevated "$HOST_AGENT_BINARY" generate-direct-control -o shuthost_direct_control

output=$(./shuthost_direct_control shutdown)

# yield to system
sleep 1

if echo "$output" | grep -q "Hopefully goodbye"; then
    printf 'Shutdown command sent successfully!\n'
else
    printf 'Shutdown command not sent\n'
    exit 1
fi

if run_as_elevated test -f /tmp/shutdown_executed; then
    printf 'Shutdown command executed successfully!\n'
else
    printf 'Warning: Shutdown command file not found, but command was sent.\n'
fi

printf 'Direct host_agent installation and direct_control test completed successfully!\n'
