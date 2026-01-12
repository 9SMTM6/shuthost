#!/bin/sh

# Test direct control script generation and execution on systemd

set -e

# Build the host_agent binary
cargo build --bin shuthost_host_agent --target x86_64-unknown-linux-gnu

# Build the container
docker build -f scripts/tests/Containerfile.systemd -t shuthost-test-systemd .

# Run the test
docker run --rm -t --privileged -v "$(pwd)":/repo shuthost-test-systemd /bin/sh -c "
cd /repo
./scripts/tests/install-and-run-direct-control.sh ./target/x86_64-unknown-linux-gnu/debug/shuthost_host_agent
"
