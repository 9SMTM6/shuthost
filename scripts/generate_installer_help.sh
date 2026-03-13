#!/bin/bash

# Script to generate help text files for installer scripts and CLI commands

set -e

OUTPUT_BASE="docs/examples/cli_help_output"

echo "Generating help texts for installer scripts and CLI commands..."

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

# Function to generate help for CLI commands
generate_cli_help() {
    local bin="$1"
    local subcommand="$2"
    local output_file="$3"
    local name="$4"
    
    echo "Generating help for $name..."
    cargo run --release --bin "$bin" $subcommand --help > "$output_file"
}

# Function to generate sanitized help for host agent install on Linux
generate_host_agent_install_linux() {
    local output_file="$1"
    
    if [[ "$(uname -s)" == "Linux" ]]; then
        echo "Generating sanitized help for host agent install (Linux)..."
        cargo run --release --bin shuthost_host_agent install --help | \
            sed '/-n, --hostname/,+1 s/\[default: .*\]/[default: <hostname>]/' | \
            sed '/-s, --shared-secret/,+1 s/\[default: .*\]/[default: <shared-secret>]/' > "$output_file"
    else
        echo "Warning: host_agent_install_linux.txt was not updated (not running on Linux)"
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

# CLI commands
generate_cli_help "shuthost_coordinator" "install" "$OUTPUT_BASE/coordinator_install.txt" "Coordinator install command"
generate_host_agent_install_linux "$OUTPUT_BASE/host_agent_install_linux.txt"

echo "Help texts generated in $OUTPUT_BASE/"
