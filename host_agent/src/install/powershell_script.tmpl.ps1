#!/usr/bin/env pwsh
# Set environment variables
$env:SHUTHOST_SHARED_SECRET = "{shuthost_shared_secret}"
$env:PORT = "{port}"
$env:SHUTDOWN_COMMAND = "{shutdown_command}"

# Extract and run the binary
$encodedBinary = "{encoded}"
$binaryBytes = [System.Convert]::FromBase64String($encodedBinary)
$tempFile = [System.IO.Path]::GetTempFileName() + ".exe"
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
    Start-Process -FilePath $tempFile -ArgumentList $argList -RedirectStandardOutput "$tempFile.log" -RedirectStandardError "$tempFile.err"
}

exit 0
