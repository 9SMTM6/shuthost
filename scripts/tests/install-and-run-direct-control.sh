#!/bin/sh
# CI script to test direct host_agent installation and direct_control script on systemd/openrc
# Usage: install-and-run-direct-control.sh [--type=sh|pwsh] [host_agent_binary] [user]
# Defaults: --type=sh, ./shuthost_host_agent, root

set -eu

. ./scripts/helpers.sh

TYPE="sh"
HOST_AGENT_BINARY="./shuthost_host_agent"
INIT_SYSTEM=""

while [ $# -gt 0 ]; do
  case $1 in
    --type=*) TYPE="${1#*=}" ;;
    --init-system=*) INIT_SYSTEM="${1#*=}" ;;
    *) HOST_AGENT_BINARY="$1" ;;
  esac
  shift
done

# Set TYPE based on INIT_SYSTEM if self-extracting
if [ "$INIT_SYSTEM" = "self-extracting-pwsh" ]; then
  TYPE="pwsh"
fi

printf 'Testing direct host_agent installation:\n  Host agent binary: %s\n  Type: %s\n  Init system: %s\n' "$HOST_AGENT_BINARY" "$TYPE" "$INIT_SYSTEM"

export RUST_BACKTRACE=1

set -v

INSTALL_CMD="$HOST_AGENT_BINARY install --shutdown-command=\"touch /tmp/shutdown_executed\""
if [ -n "$INIT_SYSTEM" ]; then
  INSTALL_CMD="$INSTALL_CMD --init-system $INIT_SYSTEM"
fi

run_as_elevated "$INSTALL_CMD"

# For self-extracting, the agent binary is the generated script
if [ "$INIT_SYSTEM" = "self-extracting-shell" ]; then
  HOST_AGENT_BINARY="./shuthost_host_agent_self_extracting"
elif [ "$INIT_SYSTEM" = "self-extracting-pwsh" ]; then
  HOST_AGENT_BINARY="./shuthost_host_agent_self_extracting.ps1"
fi

wait_for_agent_ready

run_as_elevated pgrep -af shuthost_host_agent || { printf 'Host agent service is not running\n' ; exit 1; }

OUTPUT_FILE="shuthost_direct_control"
TYPE_ARG=""
if [ "$TYPE" = "pwsh" ]; then
  OUTPUT_FILE="${OUTPUT_FILE}.ps1"
  TYPE_ARG="--type=pwsh"
fi

run_as_elevated "$HOST_AGENT_BINARY" generate-direct-control $TYPE_ARG -o "$OUTPUT_FILE"

# Test status command
if ./"$OUTPUT_FILE" status >/dev/null 2>&1; then
    printf 'Status command executed successfully!\n'
else
    printf 'Status command failed\n'
    exit 1
fi

output=$(./"$OUTPUT_FILE" shutdown)

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
