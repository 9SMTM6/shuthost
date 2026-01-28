# Agent-only Installation (detailed)

This document shows how to install the host agent only (no coordinator), the tradeoffs involved, and how to generate and use a direct-control script to wake and shutdown hosts while on the same LAN.

## Tradeoffs (short)

- **Pros:** Very easy to deploy; minimal components; quick to get shutdown/WOL capability running on a LAN.
- **Cons:** No central coordinator features (UI, leases, central logging, or easy cross-LAN control).
- **Networking:** The direct-control scripts rely on Wake-on-LAN which only works while the controller is on the same LAN. Opening access via port-forwarding is possible but not recommended for security reasons.

## Install

### Install agent

Install with the released host agent installer (recommended when you want a packaged, service-installed agent):

```bash
curl -fsSL https://github.com/9SMTM6/shuthost/releases/latest/download/shuthost_host_agent_installer.sh | sh
```

The installer detects your platform, installs the agent binary and service unit where appropriate, and creates a restrictive default configuration file.

### Generate a direct-control script

The agent can generate a small standalone control script that you can move to another machine on the same LAN to send wake/shutdown actions.

> **Behavioral difference:** The PowerShell self-extracting script (`self-extracting-pwsh`) runs attached to the service process, unlike the shell version which automatically backgrounds it with `nohup`. To background the PowerShell script, start the script itself in the background (e.g., `Start-Process -WindowStyle Hidden` on Windows or `pwsh script.ps1 &` on Unix).

Run this on the machine where the agent binary is installed:

```bash
# The agent should be in your PATH with most installations; if not, adjust the command accordingly.
sudo shuthost_host_agent generate-direct-control

# The command places `shuthost_direct_control_<hostname>` into your current working directory.
```

```bash
# Move it to the controller device and make it executable:
chmod +x shuthost_direct_control_<hostname>
# copy via scp, USB, etc.:
scp shuthost_direct_control_<hostname> user@controller:/path/to/
```

Example usage on the controller (same LAN):

```bash
./shuthost_direct_control_<hostname> shutdown
# Check status (online/offline):
./shuthost_direct_control_<hostname> status
# Wake via Wake-on-LAN (if supported):
./shuthost_direct_control_<hostname> wake
```

The direct-control script sends authenticated requests to the agent running on the host. While on the same LAN this provides a lightweight way to wake and shutdown machines without running a coordinator.

## Limitations and security notes

- WOL only works while the controller is on the same local network segment as the target host. Remote shutdowns or wakes via port-forwarding are technically possible but expand your attack surface and are strongly discouraged.
- The generated control script is effectively a client credential for the agent — treat it as sensitive material. If it is copied or leaked, an attacker could send shutdowns or wakes.
- The agent typically requires elevated privileges to issue shutdowns; ensure you trust the machine and network where the agent is installed.

## When to prefer the coordinator

- Use the coordinator when you need a UI, centralized management for multiple hosts, lease protection (preventing shutdown during backups), remote access workflows, or when you want a single hardened point of control rather than distributing control scripts.
