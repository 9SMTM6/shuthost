# ShutHost Host Agent Installer (PowerShell)
# Installs the host agent by downloading from the coordinator

param(
    [Parameter(Mandatory=$false, Position=0)]
    [string]$RemoteUrl,
    [Parameter(Mandatory=$false)]
    [string]$Port = "9090",
    [Parameter(Mandatory=$false)]
    [switch]$Help,
    [Parameter(ValueFromRemainingArguments=$true)]
    [string[]]$InstallerArgs
)

$ErrorActionPreference = 'Stop'

function Print-Help {
    Write-Host "Usage: .\host_agent.ps1 <remote_url> [-Port <port>] [-Help] [-- <install_args>]"
    Write-Host "Install ShutHost host agent from coordinator."
    Write-Host ""
    Write-Host "Parameters:"
    Write-Host "  remote_url     URL of the coordinator (required)"
    Write-Host "  -Port <port>   Port for WoL testing (default: 9090), also passed to install command"
    Write-Host "  -Help          Show this help message"
    Write-Host "  -- <args>      Additional arguments for the host agent install command (except --port)"
}

if ($Help) { Print-Help; exit 0 }

if (-not $RemoteUrl) {
    Write-Error "RemoteUrl is required"
    Print-Help
    exit 1
}

# Determine if we should accept self-signed certificates (for localhost/testing)
$hostPart = $RemoteUrl -replace '^https?://', '' -replace '/.*$', '' -replace ':.*$', ''
$curlOpts = if ($hostPart -eq 'localhost' -or $hostPart -match '^127\.') { '-k' } else { '' }

$isUnix = $PSVersionTable.Platform -eq 'Unix'
$curlCmd = if ($isUnix) { 'curl' } else { 'curl.exe' }

$scriptPath = $MyInvocation.MyCommand.Path

function Cleanup {
    Remove-Item -Path $script:FILENAME -ErrorAction SilentlyContinue
    Remove-Item -Path $scriptPath -ErrorAction SilentlyContinue
}

function Detect-Platform {
    # Detect architecture
    $arch = if ($isUnix) { uname -m } else { $env:PROCESSOR_ARCHITECTURE }
    switch ($arch) {
        "x86_64" { $script:ARCH = "x86_64" }
        "AMD64" { $script:ARCH = "x86_64" }
        "aarch64" { $script:ARCH = "aarch64" }
        "arm64" { $script:ARCH = "aarch64" }
        default {
            Write-Error "Unsupported architecture: $arch"
            exit 1
        }
    }

    # Detect OS
    if ($isUnix) {
        $os = uname -s
        switch ($os) {
            "Linux" {
                $script:PLATFORM = "linux-musl"
            }
            "Darwin" {
                $script:PLATFORM = "macos"
            }
            default {
                Write-Error "Unsupported OS: $os"
                exit 1
            }
        }
    } else {
        $script:PLATFORM = "windows"
    }

    # Set binary name
    $script:FILENAME = "shuthost_host_agent"
    if (-not $isUnix -or $script:PLATFORM -eq "windows") {
        $script:FILENAME += ".exe"
    }
}

function Test-WolPacketReachability {
    $WOL_TEST_PORT = [int]$Port + 1

    # Start the test receiver in background
    $job = Start-Job -ScriptBlock {
        param($filename, $port)
        & "./$filename" test-wol --port $port
    } -ArgumentList $script:FILENAME, $WOL_TEST_PORT

    # Give it time to start
    Start-Sleep -Seconds 1

    # Test via coordinator API
    $TEST_RESULT = & $curlCmd $curlOpts -s -X POST "$RemoteUrl/api/m2m/test_wol?port=$WOL_TEST_PORT" 2>$null
    if ($LASTEXITCODE -ne 0) {
        $TEST_RESULT = ""
    }

    # Stop the job
    Stop-Job $job -ErrorAction SilentlyContinue
    Remove-Job $job -ErrorAction SilentlyContinue

    if ($TEST_RESULT -match '"broadcast":true') {
        Write-Host "✓ Broadcast WoL packets working"
    } else {
        Write-Host "⚠️  Broadcast WoL packets failed - check firewall rules for UDP port 9"
    }
}

function Run-As-Elevated {
    param([string]$Command)

    if ($isUnix) {
        if ((id -u) -eq 0) {
            & sh -c $Command
        } elseif (Get-Command sudo -ErrorAction SilentlyContinue) {
            & sudo sh -c $Command
        } elseif (Get-Command doas -ErrorAction SilentlyContinue) {
            & doas sh -c $Command
        } else {
            Write-Error "Neither sudo nor doas found. Please install sudo or doas."
            exit 1
        }
    } else {
        # Windows
        $winIdentity = [Security.Principal.WindowsIdentity]::GetCurrent()
        $winPrincipal = New-Object Security.Principal.WindowsPrincipal($winIdentity)
        if ($winPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
            & cmd /c $Command
        } else {
            Start-Process -FilePath "cmd" -ArgumentList "/c $Command" -Verb RunAs -Wait
        }
    }
}

# Main script

try {
    Write-Host "ShutHost Host Agent Installer"
    Write-Host "============================"
    Write-Host

    Detect-Platform

    $portSpecified = $PSBoundParameters.ContainsKey('Port')

    # Parse installer args (require explicit -- separator for forwarded install args)
    $binaryArgs = @()
    if ($portSpecified) {
        $binaryArgs += "--port"
        $binaryArgs += $Port.ToString()
    }

    # Unified handling for forwarded installer args:
    # - If any InstallerArgs are provided they must start with a literal '--'.
    # - A lone '--' is allowed and means "no forwarded args".
    if ($InstallerArgs.Length -gt 0 -and $InstallerArgs[0] -ne "--") {
        Write-Error "Forwarded installer arguments must be passed after a literal -- separator."
        Print-Help
        exit 1
    }

    if ($InstallerArgs.Length -le 1) {
        $installerArgsList = @()
    } else {
        $installerArgsList = $InstallerArgs[1..($InstallerArgs.Length-1)]
    }

    foreach ($arg in $installerArgsList) {
        if ($arg -like "--port*") {
            Write-Error "--port cannot be passed via -- as it conflicts with installer option"
            exit 1
        }
        $binaryArgs += $arg
    }

    Write-Host "Downloading host_agent for $PLATFORM/$ARCH from $RemoteUrl..."

    $downloadUrl = "$RemoteUrl/download/host_agent/$PLATFORM/$ARCH"

    & $curlCmd --compressed -fL $curlOpts $downloadUrl -o $script:FILENAME

    if (-not (Test-Path $script:FILENAME)) {
        Write-Error "Failed to download binary"
        exit 1
    }

    # Make executable on Unix
    if ($isUnix) {
        & chmod +x $script:FILENAME
    }

    Test-WolPacketReachability

    # Run the installer
    $installCmd = "./$script:FILENAME install"
    if ($binaryArgs) {
        $installCmd += " " + ($binaryArgs -join " ")
    }

    Run-As-Elevated $installCmd

    Write-Host "Installation complete!"

} finally {
    Cleanup
}
