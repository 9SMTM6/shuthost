#!/bin/sh
# CI test for the coordinator installer update flow.
# This installs an older release via the enduser installer, starts the local coordinator,
# and then updates the agent through the coordinator installer.

set -eu

. ./scripts/helpers.sh

TARGET_TAG="1.6.4"
ENDUSER_INSTALLER="./scripts/enduser_installers/host_agent.sh"
COORDINATOR_INSTALLER="./scripts/coordinator_installers/host_agent.sh"
COORDINATOR_BINARY="${1:-./shuthost_coordinator}"
COORDINATOR_URL="http://127.0.0.1:8080"

printf 'Starting coordinator installer update test\n'
printf 'Installing old release %s via enduser installer\n' "$TARGET_TAG"

sh "$ENDUSER_INSTALLER" -t "$TARGET_TAG"

printf 'Waiting for old release agent to become ready...\n'
wait_for_agent_ready

printf 'Verifying old release host agent process exists\n'
run_as_elevated pgrep -af shuthost_host_agent >/dev/null 2>&1

printf 'Installing local coordinator from %s\n' "$COORDINATOR_BINARY"
run_as_elevated "$COORDINATOR_BINARY" install "$(whoami)" --port 8080 --bind 127.0.0.1

printf 'Waiting for local coordinator to become ready...\n'
wait_for_coordinator_ready 8080

printf 'Updating host agent through the coordinator installer\n'
sh "$COORDINATOR_INSTALLER" "$COORDINATOR_URL" --update

printf 'Waiting for host agent after coordinator update to become ready...\n'
wait_for_agent_ready

printf 'Verifying host agent process is still running after coordinator update\n'
run_as_elevated pgrep -af shuthost_host_agent >/dev/null 2>&1

printf 'Coordinator installer update test completed successfully!\n'
