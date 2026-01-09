#!/bin/sh

# This may be a template containing placeholders like {client_id}, {shared_secret}, and {embedded_remote_url}
# that must be replaced with actual values before use.

set -eu

print_help() {
        cat <<EOF
Usage: $0 <take|release> <host> [remote_url] [--async]

Requires: curl, openssl, date, hexdump

Arguments:
    <take|release>   Action to perform (required)
    <host>           Target host (required)
    [remote_url]     Coordinator base URL (optional)
    [--async]        Perform action asynchronously (optional, can be anywhere)

Options:
    -h, --help       Show this help message and exit

Examples:
    $0 take myhost
    $0 release myhost https://coordinator.example.com --async
    $0 --async take myhost
EOF
}

# Check for help flag (prioritized above other args)
for arg in "$@"; do
    case "$arg" in
        -h|--help)
            print_help
            exit 0
            ;;
    esac
done

# Parse arguments, allowing --async anywhere (POSIX sh compatible)
ASYNC_MODE=false
POSITIONAL=""
for arg in "$@"; do
    case "$arg" in
        --async)
            ASYNC_MODE=true
            ;;
        -h|--help)
            # Already handled above
            ;;
        *)
            POSITIONAL="$POSITIONAL '$(printf %s "$arg")'"
            ;;
    esac
done

# Reset positional parameters
eval set -- "$POSITIONAL"

if [ $# -lt 2 ]; then
    echo "Error: Missing required arguments." >&2
    print_help
    exit 1
fi

ACTION="$1"
TARGET_HOST="$2"
REMOTE_URL="${3:-"{embedded_remote_url}"}"

# Build coordinator URL with optional async parameter
COORDINATOR_URL="${REMOTE_URL}/api/m2m/lease/${TARGET_HOST}/${ACTION}"
if [ "$ASYNC_MODE" = true ]; then
    COORDINATOR_URL="${COORDINATOR_URL}?async=true"
fi

CLIENT_ID="{client_id}"
SECRET="{shared_secret}"

################## Boring setup complete ------------- Interesting stuff is starting here

# Get current timestamp (UTC)
TIMESTAMP=$(date -u +%s)

# Build the message and signature
MESSAGE="${TIMESTAMP}|${ACTION}"
SIGNATURE=$(printf "%s" "$MESSAGE" | openssl dgst -sha256 -hmac "$SECRET" -binary | hexdump -ve '/1 "%02x"')

# Combine into final X-Request header
set -v

X_REQUEST="${TIMESTAMP}|${ACTION}|${SIGNATURE}"

set -x

# Make the request
curl -sS --fail-with-body -X POST "$COORDINATOR_URL" \
  -H "X-Client-ID: $CLIENT_ID" \
  -H "X-Request: $X_REQUEST"
