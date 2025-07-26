#!/bin/sh

# Usage: ./shuthost_client_<client_id>.sh <take|release> <host> [remote_url] [--async]
# Requires: curl, openssl, date

set -eu

ACTION="$1"
TARGET_HOST="$2"
REMOTE_URL="${3:-"{embedded_remote_url}"}"

# TODO: test this option
# Check for --async flag in remaining arguments
ASYNC_MODE=false
shift 3 2>/dev/null || shift $# # Remove first 3 args, or all if less than 3
for arg in "$@"; do
    case "$arg" in
        --async)
            ASYNC_MODE=true
            ;;
    esac
done

# Build coordinator URL with optional async parameter
COORDINATOR_URL="${REMOTE_URL}/api/m2m/lease/${TARGET_HOST}/${ACTION}"
if [ "$ASYNC_MODE" = true ]; then
    COORDINATOR_URL="${COORDINATOR_URL}?async=true"
fi

CLIENT_ID="{client_id}"
SECRET="{shared_secret}"

# Get current timestamp (UTC)
TIMESTAMP=$(date -u +%s)

# Build the message and signature
MESSAGE="${TIMESTAMP}|${ACTION}"
SIGNATURE=$(printf "%s" "$MESSAGE" | openssl dgst -sha256 -hmac "$SECRET" -binary | hexdump -ve '/1 "%02x"')

# Combine into final X-Request header
X_REQUEST="${TIMESTAMP}|${ACTION}|${SIGNATURE}"

# Make the request
curl -sS --fail-with-body -X POST "$COORDINATOR_URL" \
  -H "X-Client-ID: $CLIENT_ID" \
  -H "X-Request: $X_REQUEST"
