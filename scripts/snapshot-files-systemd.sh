#!/bin/sh
# Wrapper script for snapshot-files-container.sh using systemd base image.

set -e

. ./scripts/snapshot-files-common.sh

if [ -n "$1" ]; then
    HOST_BINARY="$1"
else
    cargo build --release --bin shuthost_host_agent --target x86_64-unknown-linux-gnu
    cargo build --release --bin shuthost_coordinator --target x86_64-unknown-linux-gnu --features=include_linux_x86_64_agent
    HOST_BINARY="./target/x86_64-unknown-linux-gnu/release/shuthost_coordinator"
fi

trap cleanup EXIT

do_snapshot "docker.io/heywoodlh/systemd:latest" "apt-get update && apt-get install -y curl patch file" "./install-file-snapshots/systemd" "$HOST_BINARY" "systemctl restart shuthost_coordinator"

do_diff "./install-file-snapshots/systemd"
