# Deploying the Self-Extracting Agent on Unraid

This guide demonstrates how to deploy the ShutHost self-extracting host agent on an Unraid NAS. The self-extracting agent is designed for environments where traditional init systems (like systemd or openrc) are not available or desired, making it ideal for Unraid, which uses a custom init system.

## Prerequisites

- Unraid NAS with the User Scripts plugin installed (available from Community Applications).
- Network access to download the installer and communicate with the ShutHost coordinator.
- Note: Unraid on my setup only supports Wake-on-LAN (WOL) from sleep (S3) state, not from full power-off (S5). This is a hardware limitation, not specific to Unraid or ShutHost. If your setup supports full power-off WOL, you may not need the custom shutdown command.

## Installation Steps

1. **Install the User Scripts Plugin**:
   - In Unraid, go to the Apps tab and search for "User Scripts".
   - Install the plugin from the Community Applications.

2. **Obtain the Shared Secret and Coordinator Configuration**:
   - On your ShutHost coordinator server, run the following command to generate the configuration for the agent. Replace 'https://shuthost.example.com' with your coordinator URL:
     ```bash
     curl -fsSL https://shuthost.example.com/download/host_agent_installer.sh | sh -s https://shuthost.example.com --os=linux-musl
     ```
   - This command will output a configuration block. Copy this block.
   - Add the copied configuration block to your coordinator's configuration file (usually `coordinator_config.toml`).
   - Note the `shared_secret` value from the output, as you'll need it in the next step.

3. **Create a New User Script**:
   - Navigate to Settings > User Scripts in the Unraid web UI.
   - Click "Add New Script" and give it a name like "ShutHost Agent Startup".
   - Set the script to run "At Startup of Array" to ensure it executes when the array starts.

4. **Configure the Script**:
   - Paste the following script into the script editor. Replace `<secret>` with the shared secret you noted in step 2, and adjust the coordinator URL and port as needed:

     ```bash
     #!/bin/bash
     # ShutHost Serviceless Agent Startup Script for Unraid

     # Change to a directory with write permissions (e.g., Downloads share)
     cd /tmp/

     # Enable Wake-on-LAN on the network interface (adjust 'eth0' if your interface differs)
     ethtool -s eth0 wol g

     # Kill any existing agent processes to prevent conflicts
     pkill -f /usr/sbin/shuthost_agent
     pkill -f /usr/local/bin/shuthost_host_agent

     # Download and run the installer with serviceless mode
     # Replace 'https://shuthost.example.com' with your coordinator URL
     # Replace '<secret>' with your actual shared secret
     curl -fsSL https://shuthost.example.com/download/host_agent_installer.sh | sh -s https://shuthost.example.com --port=5757 --shared-secret=<secret> --shutdown-command="echo -n mem > /sys/power/state" --init-system=self-extracting-shell

     # Run the self-extracting agent binary
     ./shuthost_host_agent_self_extracting
     ```

5. **Save and Test the Script**:
   - Save the script.
   - Click "Run Script" to test it manually.
   - Check the script logs for any errors.
   - Verify that the agent appears in your ShutHost coordinator web UI.
   - Click "Run in background" to deploy it.
   - Verify that the host appears as online in your ShutHost coordinator web UI.

## Explanation of Script Components

- **WOL Setup**: `ethtool -s eth0 wol g` enables WOL on the network interface. This is necessary for the coordinator to wake the Unraid server.
- **Process Cleanup**: The `pkill` commands ensure no conflicting agent processes are running.
- **Installer Download**: Downloads the host agent installer from your coordinator and runs it with specific parameters:
  - `--port=5757`: Sets the port for agent-coordinator communication.
  - `--shared-secret=<secret>`: Authenticates the agent with the coordinator.
  - `--shutdown-command="echo -n mem > /sys/power/state"`: Uses a custom shutdown command to put the system into sleep (S3) state instead of full power-off, as required for WOL compatibility on many systems.
  - `--init-system=self-extracting-shell`: Configures the agent to run without relying on traditional init systems.
- **Agent Execution**: Runs the downloaded self-extracting binary to start the agent.

## Troubleshooting

- **WOL Not Working**: Ensure your motherboard and BIOS support WOL. Test WOL functionality independently of ShutHost.
- **Agent Not Connecting**: Check network connectivity, firewall settings, and verify the shared secret matches the coordinator configuration.
- **Logs**: Check Unraid system logs and the script execution logs for error messages.
