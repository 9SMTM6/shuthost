#!/bin/sh
# GitHub Actions pipeline: Build static demo for GitHub Pages
# This script builds the demo, snapshots the HTML, and infers/copies required assets.

set -ev

export_dir="target/gh-pages"

rm -rf "$export_dir"

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
mkdir -p $export_dir

base_url=http://localhost:$port

# Fetch demo HTML
curl -s $base_url/ > "$export_dir/index.html"

# Collect all internal pages to fetch
pages=$(grep -Eo 'href="/[^"]*"' "$export_dir/index.html" | sed 's/href="//;s/"$//' | grep -v '^http' | sort | uniq)

# Fetch additional pages
for page in $pages; do
    if [ "$page" != "/" ]; then
        filename="${page#/}.html"
        curl -s "$base_url$page" > "$export_dir/$filename"
    fi
done

# Adjust links in HTML files for static hosting
for html in "$export_dir"/*.html; do
    sed -i 's|href="/"|href="index.html"|g' "$html"
    sed -i 's|href="/\([^/][^"]*\)"|href="\1.html"|g' "$html"
    sed -i 's|src="/\([^/][^"]*\)"|src="\1"|g' "$html"
done

# Infer and fetch assets from demo server
for html in "$export_dir"/*.html; do
    grep -Eo '(src|href)="[^"]+"' "$html" | \
        sed -E 's/^(src|href)="//;s/"$//' | \
        while read asset; do
            # Only fetch local relative assets
            case "$asset" in
                ./*) ;;
                *) continue;;
            esac
            mkdir -p "$export_dir/$(dirname "$asset")"
            curl -s "http://localhost:8090/$asset" -o "$export_dir/$asset"
        done
done

# Fetch downloadable files (installers, scripts, binaries)
echo "Fetching downloadable files..."

agent_dir="$export_dir/download/host_agent"
mkdir -p $agent_dir

# Function to fetch downloadable files
fetch_download() {
    filename="$1"
    curl -s "$base_url/download/$filename" -o "$export_dir/download/$filename"
}

# Installers and scripts
fetch_download "host_agent_installer.sh"
fetch_download "host_agent_installer.ps1"
fetch_download "client_installer.sh"
fetch_download "client_installer.ps1"
fetch_download "shuthost_client.sh"
fetch_download "shuthost_client.ps1"

# Function to fetch agent binaries with proper error handling
fetch_agent() {
    path="$1"
    mkdir -p "$(dirname "$agent_dir/$path")"
    if curl -s -w "%{http_code}" "$base_url/download/host_agent/$path" | grep -q "^2"; then
        curl -s "$base_url/download/host_agent/$path" -o "$agent_dir/$path"
    else
        echo "$path agent not available"
    fi
}

# Host agent binaries (only fetch if they exist)
fetch_agent "macos/aarch64"
fetch_agent "macos/x86_64"
fetch_agent "linux-musl/x86_64"
fetch_agent "linux-musl/aarch64"
fetch_agent "windows/x86_64"
fetch_agent "windows/aarch64"

# Stop demo service
kill $DEMO_PID

echo "Static demo prepared in $export_dir. Ready for GitHub Pages deployment."
