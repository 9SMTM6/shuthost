#!/bin/sh
# Wrapper script for common.sh using systemd base image.

set -e

. ./scripts/snapshot_files/common.sh

# Configuration
CONTAINERFILE="scripts/snapshot_files/Containerfile.systemd"
RESTART_CMD="systemctl restart shuthost_coordinator"
STOP_CMD="systemctl stop shuthost_coordinator"
BASE_IMAGE="shuthost-systemd"
OUTPUT_DIR="./install-file-snapshots/systemd"

if [ -n "$1" ]; then
    directory="./target/x86_64-unknown-linux-gnu/debug"
    mkdir -p ${directory}
    cp "$1" "${directory}/shuthost_coordinator"
else
    cargo build --bin shuthost_host_agent --target x86_64-unknown-linux-gnu
    cargo build --bin shuthost_coordinator --target x86_64-unknown-linux-gnu --features=include_linux_x86_64_agent
fi

trap cleanup EXIT

do_snapshot

do_diff
