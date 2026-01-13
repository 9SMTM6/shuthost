#!/bin/sh

# Test direct control script generation and execution on OpenRC with pwsh

set -e

. ./scripts/helpers.sh

build_musl

# Build the container
docker build -f scripts/tests/Containerfile.alpine -t shuthost-test-alpine-pwsh .

# Run the test
docker run --rm -t --privileged -v "$(pwd)":/repo --env-file scripts/tests/coverage.env shuthost-test-alpine-pwsh /bin/sh -c "
cd /repo
./scripts/tests/install-and-run-direct-control.sh --type=pwsh ./target/x86_64-unknown-linux-musl/debug/shuthost_host_agent
"