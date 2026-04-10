#!/bin/sh
# GitHub Actions pipeline: Build static demo for GitHub Pages
# This script builds the demo, snapshots the HTML, and infers/copies required assets.

set -ev

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
cd "$SCRIPT_DIR/.."

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

port=8091
"$binary" demo-service --port $port "$subpath" &
DEMO_PID=$!

# Wait for server to start
sleep 2

# Create output directory
mkdir -p $export_dir

base_url=http://localhost:$port

# Fetch demo HTML
curl -s $base_url/ > "$export_dir/index.html"

# Infer and fetch assets from demo server (root-relative paths, before rewriting links)
for html in "$export_dir"/*.html; do
    grep -Eo '(src|href)="(/[^"]*)"' "$html" | \
        sed -E 's/^(src|href)="//;s/"$//' | \
        while read asset; do
            # Only fetch actual static files (have a file extension); skip bare SPA routes
            case "$asset" in
                *.*) ;;
                *) continue;;
            esac
            local_path="${asset#/}"
            mkdir -p "$export_dir/$(dirname "$local_path")"
            curl -s "$base_url$asset" -o "$export_dir/$local_path"
        done
done

# Rewrite root-relative links to include subpath (required for GitHub Pages deployment at a subpath)
for html in "$export_dir"/*.html; do
    sed -i "s|href=\"/\([^\"]*\)\"|href=\"${subpath}\1\"|g" "$html"
    sed -i "s|src=\"/\([^\"]*\)\"|src=\"${subpath}\1\"|g" "$html"
done

# Fetch dynamically loaded API data (not discoverable via HTML attribute scraping)
echo "Fetching API data..."

api_dir="$export_dir/api"
mkdir -p "$api_dir"
curl -s "$base_url/api/dependency-data.json" -o "$api_dir/dependency-data.json"

# Fetch service worker script explicitly (not in HTML as a src/href attribute).
# Place it under the configured subpath to match the SW registration URL
# (${demoSubpath}/sw.js, where demoSubpath is '' for '/' and '/foo' otherwise).
echo "Fetching service worker..."
if [ "$subpath" = "/" ] || [ -z "$subpath" ]; then
    sw_export_dir="$export_dir"
else
    sw_export_dir="$export_dir/${subpath#/}"
fi
mkdir -p "$sw_export_dir"
curl -s "$base_url/sw.js" -o "$sw_export_dir/sw.js"

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

# Copy index.html as 404.html so GitHub Pages serves the SPA for all deep-link paths
# (e.g. /hosts, /clients, /docs) that are handled by the client-side router.
cp "$export_dir/index.html" "$export_dir/404.html"

echo "Static demo prepared in $export_dir. Ready for GitHub Pages deployment."
