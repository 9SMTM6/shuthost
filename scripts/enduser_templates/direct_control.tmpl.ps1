#!/usr/bin/env pwsh

# This may be a template containing placeholders like {host_ip}, {port}, {shared_secret}, {mac_address}, and {hostname}
# that must be replaced with actual values before use.

param(
    [Parameter(Mandatory=$true, Position=0)]
    [ValidateSet("status", "shutdown", "wake")]
    [string]$Action
)

$HOST_IP = "{host_ip}"
$PORT = {port}
$SECRET = "{shared_secret}"
$MAC_ADDRESS = "{mac_address}"
$BROADCAST_IP = "255.255.255.255"

function Show-Help {
    $helpText = @"
Usage: $($MyInvocation.MyCommand.Name) <status|shutdown|wake>

Generated for host: {hostname}

Requires: PowerShell 6+

Arguments:
    <status|shutdown|wake>   Action to perform (required)

Options:
    -h, --help               Show this help message and exit

Examples:
    $($MyInvocation.MyCommand.Name) status
    $($MyInvocation.MyCommand.Name) shutdown
    $($MyInvocation.MyCommand.Name) wake
"@
    Write-Host $helpText
}

# Check for help
if ($PSBoundParameters.ContainsKey('Help') -or $args -contains '-h' -or $args -contains '--help') {
    Show-Help
    exit 0
}

################## Boring setup complete ------------- Interesting stuff is starting here

switch ($Action) {
    {$_ -in "status", "shutdown"} {
        # Get current timestamp (UTC)
        $timestamp = [long][Math]::Floor((Get-Date).ToUniversalTime().Subtract([DateTime]::new(1970,1,1,0,0,0,[DateTimeKind]::Utc)).TotalSeconds)

        # Build the message
        $message = "$timestamp|$Action"

        # Create HMAC-SHA256 signature
        $hmac = New-Object System.Security.Cryptography.HMACSHA256
        $hmac.Key = [System.Text.Encoding]::UTF8.GetBytes($SECRET)
        $signatureBytes = $hmac.ComputeHash([System.Text.Encoding]::UTF8.GetBytes($message))
        $signature = [BitConverter]::ToString($signatureBytes).Replace('-', '').ToLower()

        # Combine into final message
        $finalMessage = "$timestamp|$Action|$signature"

        # Send the message via TCP and print response
        try {
            $tcpClient = New-Object System.Net.Sockets.TcpClient
            $tcpClient.Connect($HOST_IP, $PORT)
            $stream = $tcpClient.GetStream()
            $writer = New-Object System.IO.StreamWriter($stream)
            $writer.Write($finalMessage)
            $writer.Flush()
            $reader = New-Object System.IO.StreamReader($stream)
            $response = $reader.ReadToEnd()
            Write-Host $response
            $tcpClient.Close()
        } catch {
            Write-Error "Failed to send TCP message: $_"
            exit 1
        }
    }
    "wake" {
        # Construct magic packet
        $packet = [byte[]]::new(102)
        # 6 bytes of FF
        for ($i = 0; $i -lt 6; $i++) {
            $packet[$i] = 0xFF
        }
        # 16 repetitions of MAC address
        $macBytes = $MAC_ADDRESS -split ':' | ForEach-Object { [Convert]::ToByte($_, 16) }
        for ($i = 0; $i -lt 16; $i++) {
            for ($j = 0; $j -lt 6; $j++) {
                $packet[6 + $i * 6 + $j] = $macBytes[$j]
            }
        }

        # Send magic packet via UDP
        try {
            $udpClient = New-Object System.Net.Sockets.UdpClient
            $udpClient.Connect($BROADCAST_IP, 9)
            $udpClient.Send($packet, $packet.Length)
            $udpClient.Close()
        } catch {
            Write-Error "Failed to send UDP packet: $_"
            exit 1
        }
    }
    default {
        Write-Error "Invalid action '$Action'. Must be status, shutdown, or wake."
        exit 1
    }
}