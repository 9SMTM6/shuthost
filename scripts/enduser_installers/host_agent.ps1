# ShutHost Host Agent Binary Installer (PowerShell)
# Works on Windows and Unix systems with PowerShell Core

param(
    [Parameter(Mandatory=$false)]
    [string]$Tag,
    [Parameter(Mandatory=$false)]
    [string]$Branch,
    [Parameter(Mandatory=$false)]
    [switch]$Help
)

# Show help if requested
if ($Help) {
    Write-Host "Usage: .\host_agent.ps1 [-Tag <tag>] [-Branch <branch>] [-Help]"
    Write-Host "Install ShutHost host agent binary."
    Write-Host "Options:"
    Write-Host "  -Tag <tag>       Specify a release tag to download."
    Write-Host "  -Branch <branch> Specify a branch; tag will be 'nightly_release<branch>'."
    Write-Host "  -Help            Show this help message."
    Write-Host "If no options, defaults to latest release."
    exit 0
}

# This script can be configured with parameters to specify a release tag or branch.

# Determine the tag
if ($Branch) {
    $Tag = "nightly_release_$Branch"
}

# Set URLs based on tag
if ($Tag) {
    $BASE_URL = "https://github.com/9SMTM6/shuthost/releases/tag/$Tag"
    $DOWNLOAD_URL = "https://github.com/9SMTM6/shuthost/releases/download/$Tag"
} else {
    $BASE_URL = "https://github.com/9SMTM6/shuthost/releases/latest/"
    $DOWNLOAD_URL = "https://github.com/9SMTM6/shuthost/releases/latest/download"
}

$isUnix = $PSVersionTable.Platform -eq 'Unix'
if ($isUnix) {
    $curlCmd = "curl"
} else {
    $curlCmd = "curl.exe"
}

function Cleanup {
    Remove-Item -Path $FILENAME -ErrorAction SilentlyContinue
    Remove-Item -Path $BINARY_NAME -ErrorAction SilentlyContinue
}

function Detect-Platform {
    # Detect architecture
    $arch = $env:PROCESSOR_ARCHITECTURE
    if ($isUnix) {
        $arch = uname -m
    }
    switch ($arch) {
        "x86_64" { $script:ARCH = "x86_64" }
        "AMD64" { $script:ARCH = "x86_64" }
        "aarch64" { $script:ARCH = "aarch64" }
        "arm64" { $script:ARCH = "aarch64" }
        default {
            Write-Error "Unsupported architecture: $arch"
            Write-Error "Supported: x86_64, aarch64"
            exit 1
        }
    }

    # Detect OS
    if ($isUnix) {
        $os = uname -s
        switch ($os) {
            "Linux" {
                $script:PLATFORM = "linux-musl"  # Prefer musl for better compatibility
                $script:TARGET_TRIPLE = "${script:ARCH}-unknown-${script:PLATFORM}"
            }
            "Darwin" {
                $script:TARGET_TRIPLE = "${script:ARCH}-apple-darwin"
            }
            default {
                Write-Error "Unsupported OS: $os"
                Write-Error "Supported: Linux, macOS (Darwin)"
                exit 1
            }
        }
    } else {
        # Windows
        $script:TARGET_TRIPLE = "${script:ARCH}-pc-windows-msvc"
    }

    # Set binary name
    if ($isUnix) {
        $script:BINARY_NAME = "shuthost_host_agent"
    } else {
        $script:BINARY_NAME = "shuthost_host_agent.exe"
    }
}

function Verify-Checksum {
    # Compute checksum
    Write-Host "Computing SHA256 checksum..."
    if ($isUnix) {
        $computedChecksum = (sha256sum $FILENAME | Select-String -Pattern '^([a-f0-9]+)' | ForEach-Object { $_.Matches[0].Groups[1].Value })
    } else {
        $computedChecksum = (Get-FileHash -Algorithm SHA256 $FILENAME).Hash.ToLower()
    }
    Write-Host "Computed checksum: $computedChecksum"
    Write-Host

    Write-Host "Please verify this checksum against the one provided for $FILENAME on the releases page:"
    Write-Host $BASE_URL
    Write-Host

    try {
        $reply = Read-Host "Have you verified the checksum? (y/N)"
        if ($reply -notmatch '^[Yy]') {
            Write-Error "Checksum verification aborted. Installation cancelled."
            exit 1
        }
    } catch {
        Write-Host "Non-interactive mode: Skipping checksum verification prompt (defaulting to yes)."
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
    Write-Host "ShutHost Host Agent Binary Installer"
    Write-Host "===================================="
    Write-Host

    Detect-Platform

    Write-Host "Detected platform: $TARGET_TRIPLE"
    Write-Host

    # Construct download URL and filename
    $FILENAME = "shuthost_host_agent-${TARGET_TRIPLE}.tar.gz"
    $DOWNLOAD_FILE_URL = "${DOWNLOAD_URL}/${FILENAME}"

    Write-Host "Downloading binary from $DOWNLOAD_FILE_URL ..."

    & $curlCmd -fLO $DOWNLOAD_FILE_URL

    Verify-Checksum

    # Extract the archive
    & tar -xzf $FILENAME

    # Run the installer
    Run-As-Elevated "./$BINARY_NAME install"

    Write-Host "Installation complete!"
    Write-Host

} finally {
    Cleanup
}