#!/bin/sh
# GitHub Actions pipeline: Build static demo for GitHub Pages
# This script builds the demo, snapshots the HTML, and infers/copies required assets.

set -e

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
cd "$SCRIPT_DIR/.."

export_dir="target/gh-pages"

port=8091

existing="$(ss -ltnp 2>/dev/null | grep -E ":$port\b" || true)"
if [ -n "$existing" ]; then
    pids="$(printf '%s
' "$existing" | sed -n 's/.*pid=\([0-9][0-9]*\).*/\1/p' | sort -u)"
    echo "Port $port is already in use by the following process(es):" >&2
    printf '%s\n' "$existing" >&2

    for pid in $pids; do
        if ps -p "$pid" >/dev/null 2>&1; then
            echo "Process info for PID $pid:" >&2
            ps -p "$pid" -o pid,comm,args >&2
        fi
    done

    if [ -n "$pids" ]; then
        echo "Ready-to-use kill command:" >&2
        echo "  kill $pids" >&2
    fi

    exit 1
fi

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

set -v

if [ -z "$binary" ]; then
    cargo build --release --bin shuthost_coordinator
    binary="./target/release/shuthost_coordinator"
fi

echo "$binary"

"$binary" demo-service --port $port "$subpath" >/dev/null 2>&1 &
DEMO_PID=$!

trap 'kill "$DEMO_PID" 2>/dev/null || true' EXIT

sleep 1 # Wait for server to start

# Create output directory
mkdir -p $export_dir

base_url=http://localhost:$port

# Function to fetch downloadable files
fetch() {
    path="$1"
    mkdir -p "$(dirname "$export_dir/$path")"

    if [ "${path#/}" != "$path" ]; then
        url="$base_url$path"
    else
        url="$base_url/$path"
    fi

    curl -fsSL "$url" -o "$export_dir/${path#/}"
}

# Fetch demo SPA HTML
fetch "hosts"

# Serve SPA as 404.html so GitHub Pages serves the SPA for all deep-link paths
# (e.g. /hosts, /clients, /docs) that are handled by the client-side router.
root_html="$export_dir/404.html"

mv "$export_dir/hosts" "$root_html"

# Infer and fetch assets from demo server (root-relative paths, before rewriting links)
grep -Eo '(src|href)="(/[^"]*)"' "$root_html" | \
    sed -E 's/^(src|href)="//;s/"$//' | \
    while read asset; do
        # Only fetch actual static files (have a file extension); skip bare SPA routes
        case "$asset" in
            *.*) ;;
            *) continue;;
        esac
        fetch "$asset"
    done

# Rewrite root-relative links to include subpath (required for GitHub Pages deployment at a subpath)
sed -i "s|href=\"/\([^\"]*\)\"|href=\"${subpath}\1\"|g" "$root_html"
sed -i "s|src=\"/\([^\"]*\)\"|src=\"${subpath}\1\"|g" "$root_html"

# Fetch dynamically loaded API data (not discoverable via HTML attribute scraping)
echo "Fetching API data..."

fetch "api/dependency-data.json"

# Fetch service worker script explicitly (not in HTML as a src/href attribute).
echo "Fetching service worker..."
fetch "sw.js"

# Fetch downloadable files (installers, scripts, binaries)
echo "Fetching downloadable files..."

agent_dir="$export_dir/download/host_agent"
mkdir -p $agent_dir

# Installers and scripts
fetch "download/host_agent_installer.sh"
fetch "download/host_agent_installer.ps1"
fetch "download/client_installer.sh"
fetch "download/client_installer.ps1"
fetch "download/shuthost_client.sh"
fetch "download/shuthost_client.ps1"

# Function to fetch agent binaries with proper error handling
fetch_agent() {
    path="$1"
    mkdir -p "$(dirname "$agent_dir/$path")"
    tmpfile="$agent_dir/$path.tmp"

    if curl -fsSL "$base_url/download/host_agent/$path" -o "$tmpfile"; then
        mv "$tmpfile" "$agent_dir/$path"
    else
        rm -f "$tmpfile"
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

echo "Static demo prepared in $export_dir. Ready for GitHub Pages deployment."
