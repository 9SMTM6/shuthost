#!/bin/sh

build_musl() {
    docker build -t shuthost-builder -f scripts/snapshot_files/build.Containerfile .
    docker run --rm -v "$(pwd):/src" shuthost-builder sh -c "cargo build --release --bin shuthost_host_agent --target x86_64-unknown-linux-musl && cargo build --release --bin shuthost_coordinator --target x86_64-unknown-linux-musl --features=include_linux_musl_x86_64_agent"
}
