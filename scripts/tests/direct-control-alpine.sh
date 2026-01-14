#!/bin/sh

# Test direct control script generation and execution on OpenRC

set -e

. ./scripts/helpers.sh

if [ -n "$1" ]; then
    directory="./target/x86_64-unknown-linux-musl/debug"
    mkdir -p ${directory}
    cp "$1" "${directory}/shuthost_host_agent"
else
    build_musl
fi

# Build the container
docker build -f scripts/tests/Containerfile.alpine -t shuthost-test-alpine .

# Run the test
docker run --rm -t --privileged -v "$(pwd)":/repo --workdir /repo --env-file scripts/tests/coverage.env shuthost-test-alpine /bin/sh -c "
./scripts/tests/install-and-run-direct-control.sh ./target/x86_64-unknown-linux-musl/debug/shuthost_host_agent
"
