#!/bin/sh

# Test direct control script generation and execution on systemd

set -e

# Check if the cargo llvm-cov subcommand exists and set up the environment
if cargo llvm-cov --help > /dev/null 2>&1; then
    eval "$(cargo llvm-cov show-env --export-prefix --remap-path-prefix)"
fi

# Build the host_agent binary
cargo build --bin shuthost_host_agent

# Build the container
docker build -f scripts/tests/Containerfile.systemd -t shuthost-test-systemd .

# Run the test
docker run --rm -t --privileged -v "$(pwd)":/repo --workdir /repo --env-file scripts/tests/coverage.env shuthost-test-systemd /bin/sh -c "
./scripts/tests/install-and-run-direct-control.sh ./target/debug/shuthost_host_agent
"
