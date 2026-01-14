#!/bin/sh

# Test direct control script generation and execution on systemd

set -e

. ./scripts/helpers.sh

if [ -n "$1" ]; then
    directory="./target/x86_64-unknown-linux-gnu/debug"
    mkdir -p ${directory}
    cp "$1" "${directory}/shuthost_host_agent"
else
    build_gnu
fi

# Build the container
docker build -f scripts/tests/Containerfile.systemd -t shuthost-test-systemd .

# Run the test
docker run --rm -t --privileged -v "$(pwd)":/repo --workdir /repo --env-file scripts/tests/coverage.env shuthost-test-systemd /bin/sh -c "
./scripts/tests/install-and-run-direct-control.sh ./target/debug/shuthost_host_agent
"
