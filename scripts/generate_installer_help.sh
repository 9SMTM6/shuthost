#!/bin/bash

# Script to generate help text files for installer scripts

set -e

OUTPUT_BASE="docs/examples/cli_help_output"

echo "Generating help texts for installer scripts..."

# Function to generate help for a script
generate_help() {
    local script_path="$1"
    local output_file="$2"
    local name="$3"
    
    echo "Generating help for $name..."
    if [[ "$script_path" == *.ps1 ]]; then
        if command -v pwsh >/dev/null 2>&1; then
            pwsh -Command "& '$script_path' -Help" > "$output_file"
        else
            echo "PowerShell not available, skipping $name"
            return 1
        fi
    else
        sh "$script_path" -h > "$output_file"
    fi
}

# Enduser installers
generate_help "scripts/enduser_installers/coordinator.sh" "$OUTPUT_BASE/enduser_installers/coordinator.txt" "Coordinator installer"
generate_help "scripts/enduser_installers/host_agent.sh" "$OUTPUT_BASE/enduser_installers/host_agent.txt" "Host agent installer"
generate_help "scripts/enduser_installers/host_agent.ps1" "$OUTPUT_BASE/enduser_installers/host_agent.ps1.txt" "Host agent installer (PowerShell)"

# Coordinator installers
generate_help "scripts/coordinator_installers/client.sh" "$OUTPUT_BASE/coordinator_installers/client.txt" "Client installer"
generate_help "scripts/coordinator_installers/host_agent.sh" "$OUTPUT_BASE/coordinator_installers/host_agent.txt" "Host agent installer (coordinator)"
generate_help "scripts/coordinator_installers/host_agent.ps1" "$OUTPUT_BASE/coordinator_installers/host_agent.ps1.txt" "Host agent installer (coordinator PowerShell)"
generate_help "scripts/coordinator_installers/client.ps1" "$OUTPUT_BASE/coordinator_installers/client.ps1.txt" "Client installer (PowerShell)"

echo "Help texts generated in $OUTPUT_BASE/"
