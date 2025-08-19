<!-- ONLY USE HTML IN THIS FILE, IT GETS INCLUDED IN THE WebGUI -->

<aside class="alert alert-warning" role="note" aria-label="Agent Installation Requirements">
    <h3 class="alert-title">⚠️ Agent Installation Requirements</h3>
    <ul>
        <li><strong>Superuser Access:</strong> The installer requires <code>curl</code> and will install as
            superuser (root/sudo/doas)</li>
        <li><strong>Unprotected Resources:</strong> Installation requires access to unprotected
            download endpoints (see security notes below)</li>
        <li><strong>Static IP Required:</strong> The host needs a static IP address for shutdown
            commands and online status monitoring</li>
        <li><strong>Manual Configuration:</strong> The generated config must be manually added
            to your coordinator configuration file</li>
    </ul>
</aside>

<aside class="alert alert-error" role="note" aria-label="Platform Limitations">
    <h3 class="alert-title">🚫 Platform Limitations</h3>
    <ul>
        <li><strong>No Windows Support:</strong> Only Unix distributions are supported.</li>
        <li><strong>No BSD Support:</strong> BSD-based systems are not currently supported.</li>
    </ul>
</aside>

<aside class="alert alert-info" role="note" aria-label="Wake-on-LAN (WOL) Requirements">
    <h3 class="alert-title">💡 Wake-on-LAN (WOL) Requirements</h3>
    <p>The agent requires Wake-on-LAN for remote startup functionality:</p>
    <ul>
        <li><strong>Motherboard Support:</strong> Your motherboard must support WOL.
            <ul>
                <li><strong>BIOS Configuration:</strong> WOL must be enabled in BIOS/UEFI settings.</li>
                <li><strong>Power State Limitation:</strong> Some systems only support WOL from sleep mode, not full shutdown.</li>
            </ul>
        </li>
        <li><strong>OS Configuration:</strong> WOL must be enabled in the operating system.</li>
        <li><strong>Network Requirements:</strong> Requires network broadcast support and host reachability.
            <ul>
                <li><em>Note: The installer tests network reachability but won't fail if the test is unsuccessful, so ensure your network supports broadcast for WOL to work properly.</em></li>
            </ul>
        </li>
    </ul>
</aside>