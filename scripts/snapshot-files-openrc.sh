#!/bin/sh
# Wrapper script for snapshot-files-container.sh using openrc base image.

set -e

# Build the binaries in a container
podman build -t shuthost-builder -f scripts/build.Containerfile .
podman run --rm -v "$(pwd):/src" shuthost-builder sh -c "cargo build --release --bin shuthost_host_agent --target x86_64-unknown-linux-musl && cargo build --release --bin shuthost_coordinator --target x86_64-unknown-linux-musl --features=include_linux_musl_x86_64_agent"

./scripts/snapshot-files-container.sh "docker.io/heywoodlh/openrc:latest" "apk update && apk add curl patch file" "./install-file-snapshots/openrc" "./target/x86_64-unknown-linux-musl/release/shuthost_coordinator" "rc-service shuthost_coordinator restart"
