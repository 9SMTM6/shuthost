#!/bin/sh
# CI script to test shuthost service installation
# Usage: coordinator_and_agent_service_installation.sh [coordinator_binary] [user]
# Defaults: ./shuthost_coordinator, root

set -eu

. ./scripts/helpers.sh

COORDINATOR_BINARY="${1:-./shuthost_coordinator}"
USER="${2:-root}"

printf 'Testing service installation:\n  Coordinator binary: %s\n  User: %s\n' "$COORDINATOR_BINARY" "$USER"

export RUST_BACKTRACE=1

set -v

printf 'Installing coordinator as service...\n'
run_as_elevated "$COORDINATOR_BINARY" install "$USER" --port 8080 --bind 127.0.0.1

wait_for_coordinator_ready 8080

printf 'Installing host agent...\n'
sh -c 'curl -fsSL http://localhost:8080/download/host_agent_installer.sh | sh -s http://localhost:8080'

wait_for_agent_ready

set +v

# Check processes (don't fail if grep finds nothing)
run_as_elevated ps aux | grep shuthost_host_agent || true

printf 'Service installation test completed successfully!\n'
