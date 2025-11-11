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

$CLIENT_ID = "{client_id}"
$SECRET = "{shared_secret}"

function Show-Help {
    $helpText = @"
Usage: $($MyInvocation.MyCommand.Name) <take|release> <host> [remote_url] [-Async]

Requires: PowerShell 5.1+ (built into Windows)

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
Write-Host "curl.exe --fail-with-body -sS -X POST $coordinatorUrl -H `"X-Client-ID: $CLIENT_ID`" -H `"X-Request: $X_REQUEST`""

# Make the request
& curl.exe --fail-with-body -sS -X POST $coordinatorUrl -H "X-Client-ID: $CLIENT_ID" -H "X-Request: $X_REQUEST"
if ($LASTEXITCODE -ne 0) { exit 1 }