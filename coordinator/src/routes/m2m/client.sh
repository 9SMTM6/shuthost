#!/bin/sh

# Usage: ./lease.sh <take|release> <node>
# Requires: curl, openssl, date

set -eu

ACTION="$1"
NODE="$2"
COORDINATOR_URL="http://your-coordinator/api/lease/${NODE}/${ACTION}"

CLIENT_ID="my-client-id"
SECRET="your-shared-secret"

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
