#!/usr/bin/env bash
set -eu
# Usage: check_lfs_fetched.sh <target-path>
target="$1"

if [ -f "$target" ]; then
  # Check for LFS pointer contents
  if head -n 5 "$target" | grep -qE 'version https://git-lfs.github.com/spec/v1|oid sha256:'; then
    echo "false"
    echo "LFS pointer present; object not downloaded" >&2
    exit 0
  fi

  # Treat suspiciously small files as pointers
  size=$(wc -c < "$target" || echo 0)
  if [ "$size" -lt 1024 ]; then
    echo "false"
    echo "File exists but is suspiciously small ($size bytes); treating as pointer" >&2
    exit 0
  fi

  echo "true"
  echo "LFS available" >&2
else
  echo "false"
  echo "LFS quota exceeded or unavailable (target $target missing)" >&2
fi

exit 0
