# Automated Backup with Kopia and ShutHost on Windows (Task Scheduler)

This example demonstrates how to set up an automated daily backup system using [Kopia](https://kopia.io/) for snapshot-based backups and ShutHost for managing host standby states. The setup ensures that the backup host is woken up before the backup and put back to sleep afterward, while providing notifications for success or failure.

## Overview

The backup process:
1. Wakes up the backup host using ShutHost
2. Runs Kopia to create snapshots of all configured sources
3. Puts the backup host back to sleep
4. Sends desktop notifications about the result

This setup uses Windows Task Scheduler for scheduling and execution.

## Prerequisites

- ShutHost coordinator and local client configured
- KopiaUI installed for the user and configured with repositories
- A backup host managed by ShutHost (referred to as "<kopia backup host>" in this example)
- PowerShell (included in Windows)
- BurntToast module (optional, for desktop notifications; install as admin with `Install-Module -Name BurntToast`)

## Backup Script: `%USERPROFILE%\bin\backup.ps1`

```powershell
# Configuration variables
$ShutHostClient = "$env:USERPROFILE\bin\shuthost_client_<unique identifier>.ps1"
$BackupHost = "<kopia backup host>"
$Kopia = "$env:USERPROFILE\AppData\Local\Programs\KopiaUI\resources\server\kopia.exe"

# Function to send notification
function Send-Notification {
    param (
        [string]$Title,
        [string]$Message,
        [string]$Type = "Info"
    )
    
    # Write to console
    $color = switch ($Type) {
        "Error" { "Red" }
        "Warning" { "Yellow" }
        default { "Green" }
    }
    Write-Host "$Title`: $Message" -ForegroundColor $color
    
    # Send toast notification using BurntToast
    try {
        New-BurntToastNotification -Text $Title, $Message -AppLogo (Get-Process -Id $PID).Path
    } catch {
        # If BurntToast fails, continue - console output is already shown
    }
}

# Wait for network for up to 60 seconds
$connected = $false
for ($i = 1; $i -le 60; $i++) {
    try {
        $null = Test-Connection -ComputerName 1.1.1.1 -Count 1 -ErrorAction Stop
        $connected = $true
        break
    } catch {
        Start-Sleep -Seconds 1
    }
}

# If network is still down after 60s, exit with notification
if (-not $connected) {
    Send-Notification -Title "Backup Failed" -Message "Network not reachable" -Type "Error"
    exit 1
}

# Run backup commands, exit on failure
try {
    & $ShutHostClient take $BackupHost
    
    & $Kopia snapshot create --all
} catch {
    Send-Notification -Title "Backup Failed" -Message "Backup command failed: $($_.Exception.Message)" -Type "Error"
    exit 1
} finally {
    # Always release the host, even if backup failed
    & $ShutHostClient release $BackupHost
}

# Notify success
Send-Notification -Title "Backup Succeeded" -Message "Backup completed successfully"
```

**Notes:**
- Set the `$ShutHostClient` and `$BackupHost` variables at the top of the script to match your setup
- The script uses try-catch for error handling - any command failure will stop execution and send a failure notification
- Network connectivity is verified before proceeding
- Console notifications are always shown; desktop toast notifications appear if BurntToast module is installed
- Ensure Kopia is in your PATH or provide the full path
- The `finally` block ensures the backup host is always released (put back to sleep), even if the Kopia backup fails

## Setup Instructions

1. **Create the backup script:**
   - Save the PowerShell script as `backup.ps1` in `%USERPROFILE%\bin\` (the directory should already contain the shuthost client)
   - Update the paths and host names as noted

2. [IN TESTING] **Create the scheduled task:**
   ```powershell
   $action = New-ScheduledTaskAction -Execute "powershell.exe" -Argument "-ExecutionPolicy Bypass -File $env:USERPROFILE\bin\backup.ps1"
   $trigger = New-ScheduledTaskTrigger -Daily -At 14:00
   $principal = New-ScheduledTaskPrincipal -UserId $env:USERNAME -LogonType Interactive
   Register-ScheduledTask -TaskName "Daily Backup" -Action $action -Trigger $trigger -Principal $principal
   ```

3. **Verify the setup:**
   - Run `Get-ScheduledTask -TaskName "Daily Backup"` in PowerShell to check if the task was created
   - Test manually: Run `powershell.exe -ExecutionPolicy Bypass -File %USERPROFILE%\bin\backup.ps1` directly

## Customization

- **Change the schedule:** Modify the trigger in Task Scheduler
- **Add more backup commands:** Extend the try block in the script
- **Different notification methods:** Replace the `Send-Notification` function with email, logging, etc.
- **Multiple backup hosts:** Add more take/release pairs for different hosts

## Troubleshooting

- Check task status: Run `Get-ScheduledTask -TaskName "Daily Backup"` in PowerShell
- View PowerShell logs: Run the script manually in PowerShell with verbose output
- Test manually: Run `powershell.exe -ExecutionPolicy Bypass -File %USERPROFILE%\bin\backup.ps1` directly
- Ensure execution policy allows script running: `Set-ExecutionPolicy RemoteSigned` (run as administrator)