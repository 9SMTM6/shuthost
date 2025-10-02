// Detect demo mode by presence of disclaimer element
const isDemoMode = !!document.getElementById('demo-mode-disclaimer');

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
    | { type: 'ConfigChanged'; payload: { hosts: string[], clients: string[] } }
    | { type: 'Initial'; payload: { hosts: string[]; clients: string[], status: Record<string, boolean>; leases: Record<string, LeaseSource[]> } }
    | { type: 'LeaseUpdate'; payload: { host: string; leases: LeaseSource[] } };

let persistedHostsList: string[] = [];
let persistedStatusMap: StatusMap = {};
let persistedLeaseMap: Record<string, LeaseSource[]> = {};
let persistedClientList: string[] = [];

/**
 * Establish and maintain a WebSocket connection to the backend API.
 * Reconnects automatically on close (with a small delay).
 */
const connectWebSocket = () => {
    if (isDemoMode) {
        DemoSim.init();
        return;
    }
    const wsProtocol = location.protocol === 'https:' ? 'wss' : 'ws';
    const socket = new WebSocket(`${wsProtocol}://${location.host}/ws`);

    socket.onopen = () => console.log('WebSocket connected');
    socket.onmessage = handleWebSocketMessage;
    socket.onclose = () => setTimeout(connectWebSocket, 2000);
};

/**
 * Handle a parsed WebSocket message and update state/UI accordingly.
 */
const handleMessage = (message: WsMessage) => {
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
};

/**
 * Handle incoming WebSocket messages and update local state/UI accordingly.
 */
const handleWebSocketMessage = (event: MessageEvent) => {
    try {
        const message = JSON.parse(event.data) as WsMessage;
        handleMessage(message);
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

/**
 * Close the mobile menu by unchecking the toggle.
 */
const closeMobileMenu = () => {
    const toggle = document.getElementById('mobile-menu-toggle') as HTMLInputElement;
    if (toggle) toggle.checked = false;
};

/**
 * Sort items into active and inactive groups, then combine with active first, sorted lexicographically.
 */
const sortActiveFirst = <T>(
    items: T[],
    isActive: (item: T) => boolean,
    getName: (item: T) => string
): T[] => {
    const active = items.filter(isActive);
    const inactive = items.filter(item => !isActive(item));
    const sortLexicographic = (a: T, b: T) => getName(a).localeCompare(getName(b));
    return [...active.toSorted(sortLexicographic), ...inactive.toSorted(sortLexicographic)];
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
    const hostList = sortActiveFirst(
        persistedHostsList,
        host => !!persistedStatusMap[host],
        host => host
    );
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

    const sortedClients = sortActiveFirst(
        clientEntries,
        ([_, leases]) => leases.length > 0,
        ([clientId, _]) => clientId
    );

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
    if (isDemoMode) {
        DemoSim.leaseAction(host, action);
        return;
    }
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
    if (isDemoMode) {
        DemoSim.resetClientLeases(clientId);
        return;
    }
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
    const validTabs = new Set(['hosts', 'clients', 'architecture']);

    const activateTab = (tabId: string, updateHash: boolean) => {
        if (!validTabs.has(tabId)) return;
        // Remove active class from all tabs and content
        tabs.forEach(t => {
            t.classList.remove('active');
            t.setAttribute('aria-selected', 'false');
        });
        document.querySelectorAll('.tab-content').forEach(content => {
            content.classList.remove('active');
        });

        // Add active class to clicked tab and corresponding content
        const tabButton = Array.from(tabs).find(t => t.dataset["tab"] === tabId);
        tabButton?.classList.add('active');
        tabButton?.setAttribute('aria-selected', 'true');
        document.getElementById(`${tabId}-tab`)?.classList.add('active');

        // Update the hash (deep link) if requested
        if (updateHash) {
            const newHash = `#${tabId}`;
            if (location.hash !== newHash) {
                // Setting location.hash will trigger the hashchange handler
                location.hash = tabId;
            }
        }
    };

    // Setup backdrop click to close mobile menu
    const backdrop = document.querySelector('.menu-backdrop');
    if (backdrop) {
        backdrop.addEventListener('click', closeMobileMenu);
    }

    tabs.forEach(tab => {
        tab.addEventListener('click', () => {
            const tabId = tab.dataset["tab"];
            if (!tabId) return;
            activateTab(tabId, true);
            // Close mobile menu after tab click
            closeMobileMenu();
        });
    });

    // Respond to URL hash changes (back/forward navigation or external links)
    const handleHashChange = () => {
        const hash = location.hash.replace('#', '');
        if (hash && validTabs.has(hash)) {
            activateTab(hash, false);
            // Close mobile menu on hash change
            closeMobileMenu();
        }
    };
    window.addEventListener('hashchange', handleHashChange);

    // Initial activation based on current hash or default to hosts
    if (location.hash && validTabs.has(location.hash.substring(1))) {
        activateTab(location.hash.substring(1), false);
    } else {
        // Ensure default is reflected when no hash present
        activateTab('hosts', false);
    }
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
    - '^/favicon.svg$'`;
    }

    if (traefikConfig) {
        traefikConfig.textContent = `# Add to your service labels
- "traefik.http.routers.shuthost-bypass.rule=Host(\`${domain}\`) && (PathPrefix(\`/download\`) || PathPrefix(\`/api/m2m\`) || Path(\`/manifest.json\`) || Path(\`/favicon.svg\`))"
- "traefik.http.routers.shuthost-bypass.priority=100"
# Remove auth middleware for bypass routes`;
    }

    if (!autheliaConfig || !traefikConfig) {
        console.info("No dynamic security exceptions found to be populated.")
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
    if (isDemoMode) {
        console.info('Demo mode enabled: UI is using simulated data.');
    }
};


// ==========================
// Demo Mode Simulation Namespace
// ==========================

namespace DemoSim {
    let leaseTimeout: ReturnType<typeof setTimeout> | null = null;
    let statusTimeout: ReturnType<typeof setTimeout> | null = null;

    export const init = () => {
        // Simulate initial push
        setTimeout(() => {
            handleMessage({
                type: "Initial",
                payload: {
                    hosts: ["archive", "tarbean"],
                    clients: [],
                    status: { tarbean: false, archive: false },
                    leases: { archive: [] }
                }
            });
        }, 500);
    }

    export const leaseAction = (host: string, action: string) => {
        if (action === "take") {
            // LeaseUpdate: WebInterface
            if (leaseTimeout) clearTimeout(leaseTimeout);
            leaseTimeout = setTimeout(() => {
                handleMessage({ type: "LeaseUpdate", payload: { host, leases: [{ type: "WebInterface" }] } });
            }, 500);
            // HostStatus: archive online
            if (statusTimeout) clearTimeout(statusTimeout);
            statusTimeout = setTimeout(() => {
                handleMessage({ type: "HostStatus", payload: { tarbean: false, archive: true } });
            }, 1200);
        } else if (action === "release") {
            // LeaseUpdate: no leases
            if (leaseTimeout) clearTimeout(leaseTimeout);
            leaseTimeout = setTimeout(() => {
                handleMessage({ type: "LeaseUpdate", payload: { host, leases: [] } });
            }, 500);
            // HostStatus: archive offline
            if (statusTimeout) clearTimeout(statusTimeout);
            statusTimeout = setTimeout(() => {
                handleMessage({ type: "HostStatus", payload: { tarbean: false, archive: false } });
            }, 1200);
        }
    }

    export const resetClientLeases = (clientId: string) => {
        // For demo, just clear all leases for that client
        Object.keys(persistedLeaseMap).forEach(host => {
            persistedLeaseMap[host] = (persistedLeaseMap[host] || []).filter(l => l.type !== 'Client' || l.value !== clientId);
        });
        updateClientsTable();
        updateHostsTable();
    }
}

document.addEventListener('DOMContentLoaded', initialize);
