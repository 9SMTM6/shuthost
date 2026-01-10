#!/bin/sh

# Test service installation on OpenRC (Alpine-like)

set -e

. ./scripts/helpers.sh

build_musl

# Build the container
docker build -f scripts/tests/Containerfile.alpine -t shuthost-test-alpine .

# Run the test
docker run --rm -t --privileged -v "$(pwd)":/repo shuthost-test-alpine /bin/sh -c "
cd /repo
./scripts/test-service-installation.sh ./target/x86_64-unknown-linux-musl/release/shuthost_coordinator root
# rc-service shuthost_host_agent status
"
