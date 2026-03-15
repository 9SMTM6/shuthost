#!/bin/bash
set -e

# Check if PowerShell is available
if ! command -v pwsh >/dev/null 2>&1; then
    echo "❌ PowerShell (pwsh) is not available. Please install PowerShell to run this test."
    exit 1
fi

echo "Testing installer help texts..."

# Run the generation script to update help files
./scripts/generate_installer_help.sh

# Check if any help files changed
if git diff --quiet docs/examples/cli_help_output/; then
    echo "✅ All installer help texts are up-to-date"
else
    echo "❌ Some help texts are outdated. Run 'just update_installer_help' to update them."
    echo "Changed files:"
    git diff --name-only docs/examples/cli_help_output/
    exit 1
fi
