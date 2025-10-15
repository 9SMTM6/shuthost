#!/bin/sh
# GitHub Actions pipeline: Build static demo for GitHub Pages
# This script builds the demo, snapshots the HTML, and infers/copies required assets.

set -e

# Build and run demo service
cargo build --release --bin shuthost_coordinator
./target/release/shuthost_coordinator demo-service --port 8090 &
DEMO_PID=$!

# Wait for server to start
sleep 2

# Create output directory
mkdir -p gh-pages

# Fetch demo HTML
curl -s http://localhost:8090/ > gh-pages/index.html

# Infer and fetch assets from demo server
grep -Eo '(src|href)="[^"]+"' gh-pages/index.html | \
    sed -E 's/^(src|href)="//;s/"$//' | \
    while read asset; do
        # Only fetch local assets (not external URLs)
        case "$asset" in
            http*|//*) continue;;
        esac
        mkdir -p "gh-pages/$(dirname "$asset")"
        curl -s "http://localhost:8090/$asset" -o "gh-pages/$asset"
    done

# Optionally fetch other static assets (SVGs, images, etc.)
for extra in architecture_simplified.svg architecture.svg favicon.svg manifest.json; do
    curl -s "http://localhost:8090/$extra" -o "gh-pages/$extra"
done

# Stop demo service
kill $DEMO_PID

echo "Static demo prepared in ./gh-pages. Ready for GitHub Pages deployment."
