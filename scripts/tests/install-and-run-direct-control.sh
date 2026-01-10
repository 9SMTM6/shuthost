#!/bin/sh
# CI script to test direct host_agent installation and direct_control script on systemd
# Usage: install-and-run-direct-control.sh [host_agent_binary] [user]
# Defaults: ./shuthost_host_agent, root

set -eu

. ./scripts/helpers.sh

HOST_AGENT_BINARY="${1:-./shuthost_host_agent}"
USER="${2:-root}"

printf 'Testing direct host_agent installation:\n  Host agent binary: %s\n  User: %s\n' "$HOST_AGENT_BINARY" "$USER"

export RUST_BACKTRACE=1

set -v

run_as_elevated "$HOST_AGENT_BINARY install --shutdown-command \"touch /tmp/shutdown_executed\""

printf 'Waiting for agent to be ready...\n'
i=1
while [ "$i" -le 30 ]; do
    if run_as_elevated pgrep -af shuthost_host_agent >/dev/null 2>&1; then
        printf 'Agent is ready!\n'
        break
    fi
    i=$((i + 1))
    sleep 1
done

run_as_elevated pgrep -af shuthost_host_agent || { printf 'Host agent service is not running\n' ; exit 1; }

"$HOST_AGENT_BINARY" generate-direct-control -o shuthost_direct_control

./shuthost_direct_control shutdown

# yield to system
sleep 1

if [ -f /tmp/shutdown_executed ]; then
    printf 'Shutdown command executed successfully!\n'
else
    printf 'Shutdown command did not execute\n'
    exit 1
fi

printf 'Direct host_agent installation and direct_control test completed successfully!\n'
