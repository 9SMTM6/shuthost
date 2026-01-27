#!/usr/bin/env pwsh

# { description }

$env:SHUTHOST_SHARED_SECRET = "{ secret }"
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
[System.IO.File]::WriteAllBytes($tempFile, $binaryBytes)

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
    $argList = @("service", "--port=$env:PORT", "--shutdown-command=$env:SHUTDOWN_COMMAND") + $args
    if ($IsLinux -or $IsMacOS) {
        # On Unix, shell out to use nohup for proper backgrounding, matching the shell script behavior
        $argString = ($argList + $args) -join ' '
        $cmd = "nohup `"$tempFile`" $argString >`"$tempFile.log`" 2>&1 &"
        & sh -c $cmd
    } else {
        # On Windows, use Start-Process with proper flags for backgrounding
        $proc = Start-Process -FilePath $tempFile -ArgumentList $argList -WindowStyle Hidden -PassThru
        # Immediately release the handle so the script can exit
        $null = $proc
    }
}
