#!/bin/sh
# Wrapper script for snapshot-files-container.sh using systemd base image.

set -e

cargo build --release --bin shuthost_host_agent --target x86_64-unknown-linux-gnu
cargo build --release --bin shuthost_coordinator --target x86_64-unknown-linux-gnu --features=include_linux_x86_64_agent

./scripts/snapshot-files-container.sh "docker.io/heywoodlh/systemd:latest" "apt-get update && apt-get install -y curl patch file" "./install-file-snapshots/systemd" "./target/x86_64-unknown-linux-gnu/release/shuthost_coordinator"
