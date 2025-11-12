# Automated Backup with Kopia and ShutHost on macOS (launchd)

This example demonstrates how to set up an automated daily backup system using [Kopia](https://kopia.io/) for snapshot-based backups and ShutHost for managing host standby states. The setup ensures that the backup host is woken up before the backup and put back to sleep afterward, while providing notifications for success or failure.

## Overview

The backup process:
1. Wakes up the backup host using ShutHost
2. Runs Kopia to create snapshots of all configured sources
3. Puts the backup host back to sleep
4. Sends desktop notifications about the result

This setup uses launchd for scheduling and execution.

## Prerequisites

- ShutHost coordinator and client configured
- Kopia installed and configured with repositories (installed via Homebrew at `/opt/homebrew/bin/kopia`)
- A backup host managed by ShutHost
- macOS with launchd

## Configuration Files

### Launch Agent: `~/Library/LaunchAgents/<your_reverse_domain>.dailybackup.plist`

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" 
    "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string><your_reverse_domain>.dailybackup</string>

    <key>ProgramArguments</key>
    <array>
        <string>/Users/<your_username>/.local/bin/backup</string>
    </array>

    <key>StartCalendarInterval</key>
    <dict>
        <key>Hour</key>
        <integer>14</integer>
        <key>Minute</key>
        <integer>00</integer>
    </dict>

    <key>StandardOutPath</key>
    <string>/tmp/backup.out</string>
    <key>StandardErrorPath</key>
    <string>/tmp/backup.err</string>

    <key>RunAtLoad</key>
    <true/>
</dict>
</plist>
```

This launch agent triggers the backup script daily at 2:00 PM. The `RunAtLoad` key ensures it runs on login.

### Backup Script: `~/.local/bin/backup`

```bash
#!/bin/sh

set -ex

# Function to send notification
notify_fail() {
    osascript -e "display notification \"$1\" with title \"Backup Failed\" sound name \"Basso\""
}

notify_success() {
    osascript -e "display notification \"$1\" with title \"Backup Succeeded\" sound name \"Glass\""
}


# Run backup commands, exit on failure
{
    ~/.local/bin/shuthost_client_<unique_ident> take <kopia backup host>
    printf "\n"
    /opt/homebrew/bin/kopia snapshot create --all
    printf "\n"
    ~/.local/bin/shuthost_client_<unique_ident> release <kopia backup host>
} || {
    notify_fail "Backup command failed"
    exit 1
}

# Notify success
notify_success "Backup completed successfully"
```

**Notes:**
- Replace `<your_reverse_domain>` with your reverse domain notation (e.g., `me.yourname` or `com.example`)
- Replace `<your_username>` with your macOS username
- Replace `shuthost_client_<unique_ident>` with your actual ShutHost client binary name
- Replace `<kopia backup host>` with the name of your backup host in ShutHost
- The script uses `set -ex` for strict error handling - any command failure will stop execution
- Desktop notifications provide feedback on backup status using `osascript`
- **Note:** The addition of notifications is untested

## Setup Instructions

1. **Install the files:**
   ```bash
   mkdir -p ~/Library/LaunchAgents ~/.local/bin
   # Copy the plist file to ~/Library/LaunchAgents/<your_reverse_domain>.dailybackup.plist
   # Copy the script to ~/.local/bin/backup
   chmod +x ~/.local/bin/backup
   ```

2. **Load the launch agent:**
   ```bash
   launchctl load ~/Library/LaunchAgents/<your_reverse_domain>.dailybackup.plist
   ```

3. **Verify the setup:**
   ```bash
   launchctl list | grep <your_reverse_domain>.dailybackup
   ```

## Customization

- **Change the schedule:** Modify the `StartCalendarInterval` in the plist file
- **Add more backup commands:** Extend the backup block in the script
- **Different notification methods:** Replace `osascript` with other notification systems
- **Multiple backup hosts:** Add more `take`/`release` pairs for different hosts

## Troubleshooting

- Check agent status: `launchctl list | grep <your_reverse_domain>.dailybackup`
- View logs: Check `/tmp/backup.out` and `/tmp/backup.err`
- Test manually: Run `~/.local/bin/backup` directly
- Unload and reload if changes are made: `launchctl unload ~/Library/LaunchAgents/<your_reverse_domain>.dailybackup.plist && launchctl load ~/Library/LaunchAgents/<your_reverse_domain>.dailybackup.plist`