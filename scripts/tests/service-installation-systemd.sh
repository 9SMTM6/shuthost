#!/bin/sh

# Test service installation on systemd (Ubuntu-like)

set -e

# Check if the cargo llvm-cov subcommand exists and set up the environment
if cargo llvm-cov --help > /dev/null 2>&1; then
    eval "$(cargo llvm-cov show-env --export-prefix --remap-path-prefix)"
fi

# Build the coordinator binary
cargo build --bin shuthost_host_agent
cp ./target/debug/shuthost_host_agent ./target/release/
cp ./target/debug/shuthost_host_agent ./target/x86_64-unknown-linux-gnu/debug/
# copy agent debug build to release path for inclusion in coordinator
cp ./target/debug/shuthost_host_agent ./target/x86_64-unknown-linux-gnu/release/
# also fake the musl agent, since some scripts only look for that...
cp ./target/debug/shuthost_host_agent ./target/x86_64-unknown-linux-musl/release/            
cargo build --bin shuthost_coordinator --features include_linux_x86_64_agent,include_linux_musl_x86_64_agent
cp ./target/debug/shuthost_coordinator ./target/release/
cp ./target/debug/shuthost_coordinator ./target/x86_64-unknown-linux-gnu/debug/
# copy coordinator debug build to release path for other scripts that expect the binary there
cp ./target/debug/shuthost_coordinator ./target/x86_64-unknown-linux-gnu/release/

# Build the container
docker build -f scripts/tests/Containerfile.systemd -t shuthost-test-systemd .

# Run the test
docker run --rm -t --privileged -v "$(pwd)":/repo --workdir /repo --env-file scripts/tests/coverage.env shuthost-test-systemd /bin/sh -c "
./scripts/tests/coordinator_and_agent_service_installation.sh ./target/debug/shuthost_coordinator
"
