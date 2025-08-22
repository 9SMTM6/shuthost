
// ==========================
// Types & State
// ==========================

/** Map of host name -> online status. */
type StatusMap = Record<string, boolean>;

/** Represents the source of a lease. */
type LeaseSource =
    | { type: 'WebInterface' }
    | { type: 'Client'; value: string };

/** WebSocket message types exchanged with the coordinator backend. */
type WsMessage = 
    | { type: 'HostStatus'; payload: Record<string, boolean> }
    | { type: 'ConfigChanged'; payload: { hosts: string[], clients: string[]} }
    | { type: 'Initial'; payload: { hosts: string[]; clients: string[], status: Record<string, boolean>; leases: Record<string, LeaseSource[]> } }
    | { type: 'LeaseUpdate'; payload: { host: string; leases: LeaseSource[] } };

type Client = {
    id: string;
    leases: string[];
};

let persistedHostsList: string[] = [];
let persistedStatusMap: StatusMap = {};
let persistedLeaseMap: Record<string, LeaseSource[]> = {};
let persistedClientList: string[] = [];

/**
 * Establish and maintain a WebSocket connection to the backend API.
 * Reconnects automatically on close (with a small delay).
 */
const connectWebSocket = () => {
    const wsProtocol = location.protocol === 'https:' ? 'wss' : 'ws';
    const socket = new WebSocket(`${wsProtocol}://${location.host}/ws`);

    socket.onopen = () => console.log('WebSocket connected');
    socket.onmessage = handleWebSocketMessage;
    socket.onclose = () => setTimeout(connectWebSocket, 2000);
};

/**
 * Handle incoming WebSocket messages and update local state/UI accordingly.
 * Wraps parsing in a try/catch to avoid uncaught exceptions from malformed messages.
 */
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
                persistedHostsList = message.payload.hosts;
                updateHostsTable();
                updateClientsTable();
                break;
            case 'HostStatus':
                persistedStatusMap = message.payload;
                updateHostsTable();
                break;
            case 'ConfigChanged':
                persistedClientList = message.payload.clients;
                persistedHostsList = message.payload.hosts;
                updateHostsTable();
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

/**
 * Return a small object describing the status text to display for a host.
 * If status is undefined the host is still loading.
 */
const getHostStatus = (hostName: string) => {
    const status = persistedStatusMap[hostName];
    return {
        statusText: status === undefined ? 'Loading...' : (status ? 'online' : 'offline')
    };
};

/**
 * Format leases for display in the hosts table.
 * Filters out the WebInterface lease (internal UI lease) and returns a comma-separated string
 * of client IDs or "None" if no client leases exist.
 */
const getFormattedLeases = (hostname: string): string => {
    // Only show client leases, not WebInterface
    const clientLeases = (persistedLeaseMap[hostname] || []).filter(lease => lease.type === 'Client');
    return clientLeases.length > 0 ? clientLeases.map(formatLeaseSource).join(', ') : 'None';
};

const hasClientsConfigured = () => persistedClientList.length > 0;

/**
 * Convert a LeaseSource to a human readable string.
 */
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

// ==========================
// Table Row Creation
// ==========================

const createHostRow = (hostName: string) => {
    const { statusText } = getHostStatus(hostName);
    const leases = getFormattedLeases(hostName);
    const clientsConfigured = hasClientsConfigured();
    return `
        <tr data-hostname="${hostName}" class="table-row" role="row">
            <th class="table-cell" scope="row">${hostName}</th>
            <td class="table-cell status" aria-label="Status">${statusText}</td>
            ${clientsConfigured ? `<td class="table-cell leases" aria-label="Leases">${leases}</td>` : ''}
            <td class="table-cell" aria-label="Actions">
                <div class="actions-cell">
                    <button 
                        class="btn btn-green take-lease" 
                        onclick="updateLease('${hostName}', 'take')" 
                        type="button"
                        aria-label="${clientsConfigured ? `Take lease for ${hostName}` : `Start ${hostName}`}" 
                    >${clientsConfigured ? "Take Lease" : "Start"}</button>
                    <button 
                        class="btn btn-red release-lease" 
                        onclick="updateLease('${hostName}', 'release')" 
                        type="button"
                        aria-label="${clientsConfigured ? `Release lease for ${hostName}` : `Shutdown ${hostName}`}" 
                    >${clientsConfigured ? "Release Lease" : "Shutdown"}</button>
                </div>
            </td>
        </tr>
    `;
};

const createClientRow = (clientId: string, leases: string[]) => {
    const hasLeases = leases.length > 0;
    return `
    <tr data-client-id="${clientId}" class="table-row" role="row">
        <th class="table-cell" scope="row">${clientId}</th>
        <td class="table-cell" aria-label="Leases">${leases.join(', ') || 'None'}</td>
        <td class="table-cell" aria-label="Actions">
            <div class="actions-cell">
                <button 
                    class="btn btn-red" 
                    onclick="resetClientLeases('${clientId}')"
                    type="button"
                    aria-label="Reset leases for ${clientId}"
                    ${!hasLeases ? 'disabled' : ''}
                >
                    Reset Leases
                </button>
            </div>
        </td>
    </tr>
    `;
};

// ==========================
// Table Update Functions
// ==========================

/**
 * Update DOM attributes and visibility for a given host table row.
 * - Updates status cell text
 * - Updates lease cell text
 * - Shows/hides take/release buttons depending on whether the WebInterface holds a lease
 */
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

/**
 * Iterate over all host rows and refresh their attributes from current persisted state.
 */
const updateNodeAttrs = () => {
    document.querySelectorAll<HTMLTableRowElement>('#host-table-body tr').forEach(row => {
        const hostname = row.dataset["hostname"];
        if (hostname) {
            updateRowAttributes(row, hostname);
        }
    });
};

const updateHostsTableHeader = () => {
    const thead = document.querySelector('#host-table-body')?.parentElement?.querySelector('thead tr');
    if (!thead) return;
    thead.innerHTML = `
        <th class="table-header">Host</th>
        <th class="table-header">Status</th>
        ${hasClientsConfigured() ? '<th class="table-header">Leases</th>' : ''}
        <th class="table-header">Actions</th>
    `;
}

/**
 * Rebuild the hosts table from persisted host list and status map.
 * Active hosts are sorted alphabetically and displayed before inactive hosts.
 */
const updateHostsTable = () => {
    const hostTableBody = document.getElementById('host-table-body');
    const activeHosts = persistedHostsList.filter((el) => persistedStatusMap[el])
    const inactiveHosts = persistedHostsList.filter((el) => !persistedStatusMap[el])

    const sortLexicographic = (hostName1: string, hostName2: string) => hostName1.localeCompare(hostName2)

    const hostList = [...activeHosts.toSorted(sortLexicographic), ...inactiveHosts.toSorted(sortLexicographic)];
    if (hostTableBody) {
        hostTableBody.innerHTML = hostList.map(createHostRow).join('');
        updateHostsTableHeader();
    }
}

/**
 * Rebuild the clients table from the persisted lease map and configured clients.
 * Groups leases by client and sorts active clients first (alphabetically), then inactive.
 */
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

    const clientEntries = Array.from(clientMap.entries());

    type ClientMapElement = [string, string[]];

    const hasActiveLeases = ([_, leases]: ClientMapElement) => leases.length > 0;

    const activeClients = clientEntries.filter(hasActiveLeases)
    const inactiveClients = clientEntries.filter((el) => !hasActiveLeases(el))

    const sortLexicographic = ([clientName1, _1]: ClientMapElement, [clientName2, _2]: ClientMapElement) => clientName1.localeCompare(clientName2)

    const sortedClients = [...activeClients.toSorted(sortLexicographic), ...inactiveClients.toSorted(sortLexicographic)];

    const clientTableBody = document.getElementById('client-table-body');
    if (clientTableBody) {
        clientTableBody.innerHTML = sortedClients
            .map(([clientId, leases]) => createClientRow(clientId, leases))
            .join('');
    }
};

// ==========================
// Backend Actions
// ==========================

/**
 * Send a lease action request to the backend.
 * action should be 'take' or 'release'.
 */
const updateLease = async (host: string, action: string) => {
    try {
        await fetch(`/api/lease/${host}/${action}`, { method: 'POST' });
    } catch (err) {
        console.error(`Failed to ${action} lease for ${host}:`, err);
    }
};

/**
 * Request the backend to clear all leases owned by a given client.
 */
const resetClientLeases = async (clientId: string) => {
    try {
        await fetch(`/api/reset_leases/${clientId}`, { method: 'POST' });
    } catch (err) {
        console.error(`Failed to reset leases for client ${clientId}:`, err);
    }
};

// ==========================
// UI Setup Functions
// ==========================

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

/**
 * Populate dynamic configuration snippets and installer commands based on current origin.
 * This keeps embedded strings in the UI in sync with where the page is served from.
 */
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
    - '^/api/m2m/(.*)$'
    - '^/manifest.json$'
    - '^/favicon.ico$'`;
    }

    if (traefikConfig) {
        traefikConfig.textContent = `# Add to your service labels
- "traefik.http.routers.shuthost-bypass.rule=Host(\`${domain}\`) && (PathPrefix(\`/download\`) || PathPrefix(\`/api/m2m\`) || Path(\`/manifest.json\`) || Path(\`/favicon.ico\`))"
- "traefik.http.routers.shuthost-bypass.priority=100"
# Remove auth middleware for bypass routes`;
    }
};

// ==========================
// Initialization
// ==========================

const initialize = () => {
    setupDynamicConfigs();
    connectWebSocket();
    setupCopyButtons();
    setupTabs();
    setupCollapsibleSections();
};

document.addEventListener('DOMContentLoaded', initialize);
