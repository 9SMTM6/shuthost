# Automated Backup with Kopia and ShutHost on Linux (systemd)

This example demonstrates how to set up an automated daily backup system using [Kopia](https://kopia.io/) for snapshot-based backups and ShutHost for managing host standby states. The setup ensures that the backup host is woken up before the backup and put back to sleep afterward, while providing notifications for success or failure.

## Overview

The backup process:
1. Wakes up the backup host using ShutHost
2. Runs Kopia to create snapshots of all configured sources
3. Puts the backup host back to sleep
4. Sends desktop notifications about the result

This setup uses systemd user services for scheduling and execution.

## Prerequisites

- ShutHost coordinator and client configured
- Kopia installed and configured with repositories
- A backup host managed by ShutHost (referred to as "<kopia backup host>" in this example)
- `notify-send` for desktop notifications (part of libnotify)

## Configuration Files

### Timer Unit: `~/.config/systemd/user/daily-backup.timer`

```ini
[Unit]
Description=Run daily backup at 14:00

[Timer]
OnCalendar=14:00
Persistent=true

[Install]
WantedBy=default.target
```

This timer triggers the backup service daily at 2:00 PM. The `Persistent=true` option ensures the timer runs even if the system was off at the scheduled time.

### Service Unit: `~/.config/systemd/user/daily-backup.service`

```ini
[Unit]
Description=Daily User Backup Job
After=network-online.target
Wants=network-online.target

[Service]
Type=oneshot
ExecStart=%h/.local/bin/backup
```

The service runs the backup script. It waits for network connectivity before starting the backup process.

### Backup Script: `~/.local/bin/backup`

```bash
#!/bin/sh
set -ex

# Configuration variables
SHUTHOST_CLIENT="$HOME/.local/bin/shuthost_client_<unique_ident>"
BACKUP_HOST="<kopia backup host>"

# Function to send notification
notify_fail() {
    notify-send "Backup Failed" "$1" -u critical
}

notify_success() {
    notify-send "Backup Succeeded" "$1" -u normal
}

# Wait for network for up to 60 seconds
for i in $(seq 1 60); do
    if ping -c1 1.1.1.1 >/dev/null 2>&1; then
        break
    fi
    sleep 1
done

# If network is still down after 60s, exit with notification
if ! ping -c1 1.1.1.1 >/dev/null 2>&1; then
    notify_fail "Network not reachable"
    exit 1
fi

# Run backup commands, exit on failure
$SHUTHOST_CLIENT take $BACKUP_HOST
trap "$SHUTHOST_CLIENT release $BACKUP_HOST" EXIT

{
    kopia snapshot create --all
} || {
    notify_fail "Backup command failed"
    exit 1
}

# Success: release and remove trap
$SHUTHOST_CLIENT release $BACKUP_HOST
trap - EXIT

# Notify success
notify_success "Backup completed successfully"
```

**Notes:**
- Set the `SHUTHOST_CLIENT` and `BACKUP_HOST` variables at the top of the script to match your setup
- The script uses `set -ex` for strict error handling - any command failure will stop execution
- Network connectivity is verified before proceeding
- Desktop notifications provide feedback on backup status
- The `trap` ensures the backup host is always released (put back to sleep), even if the Kopia backup fails

## Setup Instructions

1. **Install the files:**
   ```bash
   mkdir -p ~/.config/systemd/user ~/.local/bin
   # Copy the timer, service, and script files to their respective locations
   chmod +x ~/.local/bin/backup
   ```

2. **Enable and start the timer:**
   ```bash
   systemctl --user daemon-reload
   systemctl --user enable daily-backup.timer
   systemctl --user start daily-backup.timer
   ```

3. **Verify the setup:**
   ```bash
   systemctl --user list-timers
   ```

## Customization

- **Change the schedule:** Modify the `OnCalendar` directive in the timer file
- **Add more backup commands:** Extend the backup block in the script
- **Different notification methods:** Replace `notify-send` with email, Slack, etc.
- **Multiple backup hosts:** Add more `take`/`release` pairs for different hosts

## Troubleshooting

- Check service status: `systemctl --user status daily-backup.service`
- View logs: `journalctl --user -u daily-backup.service`
- Test manually: Run `~/.local/bin/backup` directly