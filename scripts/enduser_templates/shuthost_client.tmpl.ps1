#!/usr/bin/env pwsh

# This may be a template containing placeholders like {client_id}, {shared_secret}, and {embedded_remote_url}
# that must be replaced with actual values before use.

param(
    [Parameter(Mandatory=$true, Position=0)]
    [ValidateSet("take", "release")]
    [string]$Action,

    [Parameter(Mandatory=$true, Position=1)]
    [string]$TargetHost,

    [Parameter(Position=2)]
    [string]$RemoteUrl = "{embedded_remote_url}",

    [switch]$Async
)

$ErrorActionPreference = 'Stop'

$CLIENT_ID = "{client_id}"
$SECRET = "{shared_secret}"

$curlCmd = if ($isUnix) { "curl" } else { "curl.exe" }

function Show-Help {
    $helpText = @"
Usage: $($MyInvocation.MyCommand.Name) <take|release> <host> [remote_url] [-Async]

Requires: PowerShell 6+ (on Linux/macOS or Windows)

Arguments:
    <take|release>   Action to perform (required)
    <host>           Target host (required)
    [remote_url]     Coordinator base URL (optional)
    [-Async]         Perform action asynchronously (optional)

Options:
    -h, --help       Show this help message and exit

Examples:
    $($MyInvocation.MyCommand.Name) take myhost
    $($MyInvocation.MyCommand.Name) release myhost https://coordinator.example.com -Async
    $($MyInvocation.MyCommand.Name) -Async take myhost
"@
    Write-Host $helpText
}

# Check for help
if ($PSBoundParameters.ContainsKey('Help') -or $args -contains '-h' -or $args -contains '--help') {
    Show-Help
    exit 0
}

################## Boring setup complete ------------- Interesting stuff is starting here

# Get current timestamp (UTC)
$timestamp = [long][Math]::Floor((Get-Date).ToUniversalTime().Subtract([DateTime]::new(1970,1,1,0,0,0,[DateTimeKind]::Utc)).TotalSeconds)

# Build the message
$message = "$timestamp|$Action"

# Create HMAC-SHA256 signature
$hmac = New-Object System.Security.Cryptography.HMACSHA256
$hmac.Key = [System.Text.Encoding]::UTF8.GetBytes($SECRET)
$signatureBytes = $hmac.ComputeHash([System.Text.Encoding]::UTF8.GetBytes($message))
$signature = [BitConverter]::ToString($signatureBytes).Replace('-', '').ToLower()

# Combine into final X-Request header
$xRequest = "$timestamp|$Action|$signature"

# Build coordinator URL with optional async parameter
$coordinatorUrl = "$RemoteUrl/api/m2m/lease/$TargetHost/$Action"
if ($Async) {
    $coordinatorUrl += "?async=true"
}

# Output request details (equivalent to bash set -v/-x)
Write-Host "$curlCmd --fail-with-body -sS -X POST $coordinatorUrl -H `"X-Client-ID: $CLIENT_ID`" -H `"X-Request: $xRequest`""

# Make the request
& $curlCmd --fail-with-body -sS -X POST $coordinatorUrl -H "X-Client-ID: $CLIENT_ID" -H "X-Request: $xRequest"
if ($LASTEXITCODE -ne 0) { exit 1 }