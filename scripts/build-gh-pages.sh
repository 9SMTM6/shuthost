#!/bin/sh
# GitHub Actions pipeline: Build static demo for GitHub Pages
# This script builds the demo, snapshots the HTML, and infers/copies required assets.

set -ev

rm -rf gh-pages

# Build and run demo service
binary=""
subpath="/"
for arg in "$@"; do
    case $arg in
        --provided-binary=*)
            binary="${arg#*=}"
            ;;
        --serve-subpath=*)
            subpath="${arg#*=}"
            ;;
        *)
            echo "Unknown option: $arg"
            exit 1
            ;;
    esac
done

if [ -z "$binary" ]; then
    cargo build --release --bin shuthost_coordinator
    binary="./target/release/shuthost_coordinator"
fi

echo "$binary"

port=8090
"$binary" demo-service --port $port "$subpath" &
DEMO_PID=$!

# Wait for server to start
sleep 2

# Create output directory
mkdir -p gh-pages

base_url=http://localhost:$port

# Fetch demo HTML
curl -s $base_url/ > gh-pages/index.html

# Collect all internal pages to fetch
pages=$(grep -Eo 'href="/[^"]*"' gh-pages/index.html | sed 's/href="//;s/"$//' | grep -v '^http' | sort | uniq)

# Fetch additional pages
for page in $pages; do
    if [ "$page" != "/" ]; then
        filename="${page#/}.html"
        curl -s "$base_url$page" > "gh-pages/$filename"
    fi
done

# Adjust links in HTML files for static hosting (skip root path /)
for html in gh-pages/*.html; do
    sed -i 's|href="/\([^/][^"]*\)"|href="\1.html"|g' "$html"
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

# Fetch downloadable files (installers, scripts, binaries)
echo "Fetching downloadable files..."
mkdir -p gh-pages/download/host_agent/macos
mkdir -p gh-pages/download/host_agent/linux
mkdir -p gh-pages/download/host_agent/linux-musl

# Function to fetch downloadable files
fetch_download() {
    filename="$1"
    curl -s "$base_url/download/$filename" -o "gh-pages/download/$filename"
}

# Installers and scripts
fetch_download "host_agent_installer.sh"
fetch_download "client_installer.sh"
fetch_download "client_installer.ps1"
fetch_download "shuthost_client.sh"
fetch_download "shuthost_client.ps1"

# Function to fetch agent binaries with proper error handling
fetch_agent() {
    path="$1"
    echo "Fetching $path agent..."
    if curl -s -w "%{http_code}" "$base_url/download/host_agent/$path" | grep -q "^2"; then
        curl -s "$base_url/download/host_agent/$path" -o "gh-pages/download/host_agent/$path"
    else
        echo "$path agent not available"
    fi
}

# Host agent binaries (only fetch if they exist)
fetch_agent "macos/aarch64"
fetch_agent "macos/x86_64"
fetch_agent "linux/x86_64"
fetch_agent "linux/aarch64"
fetch_agent "linux-musl/x86_64"
fetch_agent "linux-musl/aarch64"

# Stop demo service
kill $DEMO_PID

echo "Static demo prepared in ./gh-pages. Ready for GitHub Pages deployment."
