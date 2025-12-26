#!/bin/sh
# GitHub Actions pipeline: Build static demo for GitHub Pages
# This script builds the demo, snapshots the HTML, and infers/copies required assets.

set -ev

rm -rf gh-pages

# Build and run demo service
cargo build --release --bin shuthost_coordinator
./target/release/shuthost_coordinator demo-service --port 8090 "${1:-"/"}" &
DEMO_PID=$!

# Wait for server to start
sleep 2

# Create output directory
mkdir -p gh-pages

# Fetch demo HTML
curl -s http://localhost:8090/ > gh-pages/index.html

# Collect all internal pages to fetch
pages=$(grep -Eo 'href="/[^"]*"' gh-pages/index.html | sed 's/href="//;s/"$//' | grep -v '^http' | sort | uniq)

# Fetch additional pages
for page in $pages; do
    if [ "$page" != "/" ]; then
        filename="${page#/}.html"
        curl -s "http://localhost:8090$page" > "gh-pages/$filename"
    fi
done

# Adjust links in HTML files for static hosting
for html in gh-pages/*.html; do
    sed -i 's|href="/\([^"]*\)"|href="\1.html"|g' "$html"
done

# Infer and fetch assets from demo server
for html in gh-pages/*.html; do
    grep -Eo '(src|href)="[^"]+"' "$html" | \
        sed -E 's/^(src|href)="//;s/"$//' | \
        while read asset; do
            # Only fetch local relative assets
            case "$asset" in
                ./*) ;;
                *) continue;;
            esac
            mkdir -p "gh-pages/$(dirname "$asset")"
            curl -s "http://localhost:8090/$asset" -o "gh-pages/$asset"
        done
done

# Stop demo service
kill -9 $DEMO_PID

echo "Static demo prepared in ./gh-pages. Ready for GitHub Pages deployment."
