#!/usr/bin/env pwsh

# { description }
#
# NOTE: Unlike the shell script version, this PowerShell script does NOT automatically
# background the service process. When running the service, this script will attach to
# the process. To run in the background, start THIS SCRIPT in the background instead:
#   - Windows: Start-Process -WindowStyle Hidden -FilePath pwsh -ArgumentList "-File", "script.ps1"
#   - Unix:    pwsh script.ps1 &

$env:SHUTHOST_SHARED_SECRET = "{ secret }"
$env:SHUTHOST_HOSTNAME = "{ hostname }"
$env:PORT = "{ port }"
$env:SHUTDOWN_COMMAND = "{ shutdown_command }"

# Extract and run the binary
$encodedBinary = "{ encoded }"
$binaryBytes = [System.Convert]::FromBase64String($encodedBinary)

# Stable path on Windows, since Windows firewall rules are tied to the executable path
if (-not ($IsLinux -or $IsMacOS)) {
    $stableDir = Join-Path $env:APPDATA "shuthost"
    if (-not (Test-Path $stableDir)) { New-Item -ItemType Directory -Path $stableDir | Out-Null }
    $tempFile = Join-Path $stableDir "host_agent.exe"
} else {
    $tempFile = [System.IO.Path]::GetTempFileName() + ".exe"
}

# Try to write the binary, but warn if it fails due to file locking
try {
    [System.IO.File]::WriteAllBytes($tempFile, $binaryBytes)
} catch {
    if (-not ($IsLinux -or $IsMacOS) -and $tempFile -eq (Join-Path $env:APPDATA "shuthost\host_agent.exe")) {
        Write-Warning "The host_agent executable at '$tempFile' is currently in use by a running process. The executable could not be updated and might be out of date."
    } else {
        throw  # Re-throw for other errors
    }
}

# Make executable on Unix-like systems
if ($IsLinux -or $IsMacOS) {
    & chmod +x $tempFile
}

# Run the binary
if ($args.Count -gt 0 -and -not $args[0].StartsWith("-")) {
    if ($args[0] -eq "generate-direct-control" -or $args[0] -eq "registration") {
        & $tempFile ($args + @("--script-path", $MyInvocation.MyCommand.Path, "--init-system", "self-extracting-pwsh"))
    } else {
        & $tempFile $args
    }
} else {
    # Run the service attached to this script
    # Unlike the shell script, we don't background here - the caller should background this script instead
    & $tempFile service --port=$env:PORT --shutdown-command=$env:SHUTDOWN_COMMAND --hostname=$env:SHUTHOST_HOSTNAME @args
}
