#!/bin/sh

# Test service installation on systemd (Ubuntu-like)

set -e

# Build the coordinator binary
cargo build --release --bin shuthost_host_agent --target x86_64-unknown-linux-gnu
cargo build --release --bin shuthost_coordinator --target x86_64-unknown-linux-gnu --features include_linux_x86_64_agent

# Build the container
docker build -f scripts/tests/Containerfile.systemd -t shuthost-test-systemd .

# Run the test
docker run --rm -t --privileged -v "$(pwd)":/repo --env-file scripts/tests/coverage.env shuthost-test-systemd /bin/sh -c "
cd /repo
./scripts/tests/coordinator_and_agent_service_installation.sh ./target/x86_64-unknown-linux-gnu/release/shuthost_coordinator
"
