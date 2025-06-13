// Reusable types
type Host = { name: string };
type StatusMap = Record<string, boolean>;

type LeaseSource =
    | { type: 'WebInterface' }
    | { type: 'Client'; value: string };

type WsMessage = 
    | { type: 'HostStatus'; payload: Record<string, boolean> }
    | { type: 'ConfigChanged'; payload: { hosts: string[], clients: string[]} }
    | { type: 'Initial'; payload: { hosts: string[]; clients: string[], status: Record<string, boolean>; leases: Record<string, LeaseSource[]> } }
    | { type: 'LeaseUpdate'; payload: { host: string; leases: LeaseSource[] } };

type Client = {
    id: string;
    leases: string[];
};


// Persist statusMap globally
let persistedStatusMap: StatusMap = {};
let persistedLeaseMap: Record<string, LeaseSource[]> = {};
let persistedClientList: string[] = [];

const connectWebSocket = () => {
    const wsProtocol = location.protocol === 'https:' ? 'wss' : 'ws';
    const socket = new WebSocket(`${wsProtocol}://${location.host}/ws`);

    socket.onopen = () => console.log('WebSocket connected');
    socket.onmessage = handleWebSocketMessage;
    socket.onclose = () => setTimeout(connectWebSocket, 2000);
};

const handleWebSocketMessage = (event: MessageEvent) => {
    try {
        const message = JSON.parse(event.data) as WsMessage;
        const hostTableBody = document.getElementById('host-table-body');
        if (!hostTableBody) throw new Error('Missing required element #host-table-body');
        const clientTableBody = document.getElementById('client-table-body');
        if (!clientTableBody) return;

        switch (message.type) {
            case 'Initial':
                persistedStatusMap = message.payload.status;
                persistedLeaseMap = message.payload.leases;
                persistedClientList = message.payload.clients;
                const hosts = message.payload.hosts.map(name => ({ name }));
                renderHostsTable(hosts);
                updateClientsTable();
                break;
            case 'HostStatus':
                persistedStatusMap = message.payload;
                break;
            case 'ConfigChanged':
                persistedClientList = message.payload.clients;
                const newHosts = message.payload.hosts.map(name => ({ name }));
                renderHostsTable(newHosts);
                updateClientsTable();
                break;
            case 'LeaseUpdate':
                const { host, leases } = message.payload;
                persistedLeaseMap[host] = leases;
                updateClientsTable();
                console.log(`Updated leases for ${host}:`, persistedLeaseMap[host]);
                break;
        }
        updateNodeAttrs();
    } catch (err) {
        console.error('Error handling WS message:', err);
    }
};

const getHostStatus = (hostname: string) => {
    const status = persistedStatusMap[hostname];
    return {
        statusText: status === undefined ? 'Loading...' : (status ? 'online' : 'offline')
    };
};

// Helper function to get formatted leases for a host
const getFormattedLeases = (hostname: string): string => {
    // Only show client leases, not WebInterface
    const clientLeases = (persistedLeaseMap[hostname] || []).filter(lease => lease.type === 'Client');
    return clientLeases.length > 0 ? clientLeases.map(formatLeaseSource).join(', ') : 'None';
};

// Helper to check if there are any clients configured
const hasClientsConfigured = () => persistedClientList.length > 0;

// Helper function to update a single row's attributes
const updateRowAttributes = (row: HTMLTableRowElement, hostname: string) => {
    const { statusText } = getHostStatus(hostname);
    const leases = persistedLeaseMap[hostname] || [];
    const hasWebInterfaceLease = leases.some(lease => lease.type === 'WebInterface');

    const statusCell = row.querySelector<HTMLElement>('.status');
    const leaseCell = row.querySelector<HTMLElement>('.leases');
    const takeLeaseButton = row.querySelector<HTMLButtonElement>('.take-lease');
    const releaseLeaseButton = row.querySelector<HTMLButtonElement>('.release-lease');

    if (statusCell) statusCell.textContent = statusText;
    if (leaseCell) leaseCell.textContent = getFormattedLeases(hostname);

    if (takeLeaseButton) {
        takeLeaseButton.style.display = hasWebInterfaceLease ? 'none' : 'inline-block';
    }
    if (releaseLeaseButton) {
        releaseLeaseButton.style.display = hasWebInterfaceLease ? 'inline-block' : 'none';
    }
};

// Updated updateNodeAttrs function
const updateNodeAttrs = () => {
    document.querySelectorAll<HTMLTableRowElement>('#host-table-body tr').forEach(row => {
        const hostname = row.dataset["hostname"];
        if (hostname) {
            updateRowAttributes(row, hostname);
        }
    });
};

// Updated createHostRow function
const createHostRow = (host: Host) => {
    const { statusText } = getHostStatus(host.name);
    const leases = getFormattedLeases(host.name);
    const clientsConfigured = hasClientsConfigured();
    return `
        <tr data-hostname="${host.name}" class="table-row">
            <td class="table-cell">${host.name}</td>
            <td class="table-cell status">${statusText}</td>
            ${clientsConfigured ? `<td class="table-cell leases">${leases}</td>` : ''}
            <td class="table-cell">
                <div class="actions-cell">
                    <button class="btn btn-green take-lease" onclick="updateLease('${host.name}', 'take')">${clientsConfigured ? "Take Lease" : "Start"}</button>
                    <button class="btn btn-red release-lease" onclick="updateLease('${host.name}', 'release')">${clientsConfigured ? "Release Lease" : "Shutdown"}</button>
                </div>
            </td>
        </tr>
    `;
};

const updateLease = async (host: string, action: string) => {
    try {
        await fetch(`/api/lease/${host}/${action}`, { method: 'POST' });
    } catch (err) {
        console.error(`Failed to ${action} lease for ${host}:`, err);
    }
};

const setupCopyButtons = () => {
    document.querySelectorAll<HTMLButtonElement>('.copy-button').forEach(button => {
        button.addEventListener('click', () => {
            const targetSelector = button.dataset["copyTarget"];
            if (targetSelector) {
                const target = document.querySelector<HTMLElement>(targetSelector)?.textContent;
                if (target) {
                    navigator.clipboard.writeText(target).then(() => {
                        button.textContent = "Copied!";
                        setTimeout(() => (button.textContent = "Copy"), 1500);
                    });
                }
            }
        });
    });
};

const setupTabs = () => {
    const tabs = document.querySelectorAll<HTMLButtonElement>('.tab');
    
    tabs.forEach(tab => {
        tab.addEventListener('click', () => {
            // Remove active class from all tabs and content
            tabs.forEach(t => t.classList.remove('active'));
            document.querySelectorAll('.tab-content').forEach(content => {
                content.classList.remove('active');
            });

            // Add active class to clicked tab and corresponding content
            tab.classList.add('active');
            const tabId = tab.dataset["tab"];
            document.getElementById(`${tabId}-tab`)?.classList.add('active');
        });
    });
};

const setupCollapsibleSections = () => {
    document.querySelectorAll<HTMLElement>('.collapsible-header').forEach(header => {
        header.addEventListener('click', () => {
            const targetId = header.dataset["target"];
            if (targetId) {
                const content = document.getElementById(targetId);
                const icon = header.querySelector('.collapsible-icon');
                
                content?.classList.toggle('expanded');
                icon?.classList.toggle('expanded');
            }
        });
    });
};

const setupDynamicConfigs = () => {
    const baseUrl = window.location.origin;

    // Install commands
    const hostInstallCommand = document.getElementById('host-install-command');
    const clientInstallCommand = document.getElementById('client-install-command');

    if (!hostInstallCommand || !clientInstallCommand) {
        throw new Error('Missing required install command elements');
    }

    hostInstallCommand.textContent = `curl -fsSL ${baseUrl}/download/host_agent_installer.sh | sh -s ${baseUrl} --port 5757`;
    clientInstallCommand.textContent = `curl -fsSL ${baseUrl}/download/client_installer.sh | sh -s ${baseUrl}`;

    // Configuration examples with replaced domain/backend
    const autheliaConfig = document.getElementById('authelia-config');
    const traefikConfig = document.getElementById('traefik-config');

    const domain = baseUrl.replace(/^https?:\/\//, '');

    if (autheliaConfig) {
        autheliaConfig.textContent = `- domain: ${domain}
  policy: bypass
  resources:
    - '^/download/(.*)'
    - '^/api/m2m/(.*)$'`;
    }

    if (traefikConfig) {
        traefikConfig.textContent = `# Add to your service labels
- "traefik.http.routers.shuthost-bypass.rule=Host(\`${domain}\`) && PathPrefix(\`/download\`, \`/api/m2m\`)"
- "traefik.http.routers.shuthost-bypass.priority=100"
# Remove auth middleware for bypass routes`;
    }
};

const initialize = () => {
    setupDynamicConfigs();
    connectWebSocket();
    setupCopyButtons();
    setupTabs();
    setupCollapsibleSections();
};

document.addEventListener('DOMContentLoaded', initialize);

const formatLeaseSource = (lease: LeaseSource): string => {
    switch (lease.type) {
        case 'WebInterface':
            // Warn if this is called for hosts table or clients table, as it should be filtered out before
            console.warn('formatLeaseSource called with WebInterface lease. This should be filtered out before display.');
            return '';
        case 'Client':
            return lease.value;
    }
};

const createClientRow = (clientId: string, leases: string[]) => {
    const hasLeases = leases.length > 0;
    return `
    <tr data-client-id="${clientId}" class="table-row">
        <td class="table-cell">${clientId}</td>
        <td class="table-cell">${leases.join(', ') || 'None'}</td>
        <td class="table-cell">
            <div class="actions-cell">
                <button 
                    class="btn btn-red" 
                    onclick="resetClientLeases('${clientId}')"
                    ${!hasLeases ? 'disabled' : ''}
                >
                    Reset Leases
                </button>
            </div>
        </td>
    </tr>
    `;
};

const updateClientsTable = () => {
    const clientMap = new Map<string, string[]>();

    // Group leases by client
    Object.entries(persistedLeaseMap).forEach(([host, leases]) => {
        leases.forEach(lease => {
            if (lease.type === 'Client') {
                const clientLeases = clientMap.get(lease.value) || [];
                clientLeases.push(host);
                clientMap.set(lease.value, clientLeases);
            }
        });
    });

    // Ensure all known clients are included
    persistedClientList.forEach(clientId => {
        if (!clientMap.has(clientId)) {
            clientMap.set(clientId, []);
        }
    });

    // Sort: active clients (with leases) first, then inactive (no leases), both alphabetically
    const sortedClients = Array.from(clientMap.entries())
        .sort((a, b) => {
            const aActive = a[1].length > 0;
            const bActive = b[1].length > 0;
            if (aActive && !bActive) return -1;
            if (!aActive && bActive) return 1;
            // Both active or both inactive: sort alphabetically by clientId
            return a[0].localeCompare(b[0]);
        });

    const clientTableBody = document.getElementById('client-table-body');
    if (clientTableBody) {
        clientTableBody.innerHTML = sortedClients
            .map(([clientId, leases]) => createClientRow(clientId, leases))
            .join('');
    }
};

// Helper to update hosts table header for simplified UI
function updateHostsTableHeader() {
    const thead = document.querySelector('#host-table-body')?.parentElement?.querySelector('thead tr');
    if (!thead) return;
    thead.innerHTML = `
        <th class="table-header">Host</th>
        <th class="table-header">Status</th>
        ${hasClientsConfigured() ? '<th class="table-header">Leases</th>' : ''}
        <th class="table-header">Actions</th>
    `;
}

// Patch: call updateHostsTableHeader after table is rendered
function renderHostsTable(hosts: Host[]) {
    const hostTableBody = document.getElementById('host-table-body');
    if (hostTableBody) {
        hostTableBody.innerHTML = hosts.map(createHostRow).join('');
        updateHostsTableHeader();
    }
}

// Add this function to handle resetting client leases
const resetClientLeases = async (clientId: string) => {
    try {
        await fetch(`/api/client/${clientId}/reset-leases`, { method: 'POST' });
    } catch (err) {
        console.error(`Failed to reset leases for client ${clientId}:`, err);
    }
};
