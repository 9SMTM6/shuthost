#!/bin/sh

# Usage: ./lease.sh <take|release> <node> [remote_url]
# Requires: curl, openssl, date

set -eu

ACTION="$1"
NODE="$2"
REMOTE_URL="${3:-"{embedded_remote_url}"}"
COORDINATOR_URL="${REMOTE_URL}/api/lease/${NODE}/${ACTION}"

CLIENT_ID="{client_id}"
SECRET="{shared_secret}"

# Get current timestamp (UTC)
TIMESTAMP=$(date -u +%s)

# Build the message and signature
MESSAGE="${TIMESTAMP}|${ACTION}"
SIGNATURE=$(printf "%s" "$MESSAGE" | openssl dgst -sha256 -hmac "$SECRET" -binary | openssl base64)

# Combine into final X-Request header
X_REQUEST="${TIMESTAMP}|${ACTION}|${SIGNATURE}"

# Make the request
curl -sSf -X POST "$COORDINATOR_URL" \
  -H "X-Client-ID: $CLIENT_ID" \
  -H "X-Request: $X_REQUEST"
