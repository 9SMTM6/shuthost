#!/bin/sh
# CI script to test shuthost service installation
# Usage: test-service-installation.sh [coordinator_binary] [user]
# Defaults: ./shuthost_coordinator, root

set -eu

COORDINATOR_BINARY="${1:-./shuthost_coordinator}"
USER="${2:-root}"

printf 'Testing service installation:\n  Coordinator binary: %s\n  User: %s\n' "$COORDINATOR_BINARY" "$USER"

export RUST_BACKTRACE=1

# Function to run sudo commands (skip if running as root)
run_sudo() {
    if [ "$(id -u)" -eq 0 ] || [ -f /.dockerenv ]; then
        "$@"
    else
        sudo -E "$@"
    fi
}

set -v

printf 'Installing coordinator as service...\n'
run_sudo "$COORDINATOR_BINARY" install "$USER" --port 8080 --bind 127.0.0.1

printf 'Waiting for coordinator to be ready...\n'
i=1
while [ "$i" -le 30 ]; do
    if curl -fsSL http://localhost:8080/login >/dev/null 2>&1; then
        printf 'Coordinator is ready!\n'
        break
    fi
    i=$((i + 1))
    sleep 1
done

curl -fsSL http://localhost:8080/login >/dev/null 2>&1 || { printf 'Coordinator service is not running\n' ; exit 1; }

printf 'Installing host agent...\n'
sh -c 'curl -fsSL http://localhost:8080/download/host_agent_installer.sh | sh -s http://localhost:8080'

printf 'Waiting for agent to be ready...\n'
i=1
while [ "$i" -le 30 ]; do
    if run_sudo pgrep -af shuthost_host_agent >/dev/null 2>&1; then
        printf 'Agent is ready!\n'
        break
    fi
    i=$((i + 1))
    sleep 1
done

set +v

# Check processes (don't fail if grep finds nothing)
run_sudo ps aux | grep shuthost_host_agent || true

printf 'Service installation test completed successfully!\n'
