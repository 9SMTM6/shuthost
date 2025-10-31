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

// Global WebSocket reference for bfcache handling
let currentSocket: WebSocket | null = null;

/**
 * Establish and maintain a WebSocket connection to the backend API.
 * Reconnects automatically on close (with a small delay).
 */
const connectWebSocket = () => {
    if (DemoMode.isActive) {
        DemoMode.init();
        return;
    }

    // If already connected, no need to reconnect
    if (currentSocket && currentSocket.readyState === WebSocket.OPEN) {
        console.info('WebSocket already connected');
        return;
    }

    // Close any existing connection
    if (currentSocket) {
        currentSocket.close();
    }

    const wsProtocol = location.protocol === 'https:' ? 'wss' : 'ws';
    const url = `${wsProtocol}://${location.host}/ws`;
    console.info('Attempting WebSocket connect to', url);
    const socket = new WebSocket(url);
    currentSocket = socket;

    socket.onopen = () => console.info('WebSocket connected to', url);
    socket.onmessage = handleWebSocketMessage;
    socket.onerror = (ev) => {
        // `ev` is typically an Event without much detail; still log to help
        // spot timing or repeated failures.
        console.error('WebSocket error', ev);
    };
    socket.onclose = (ev) => {
        console.warn('WebSocket closed', { code: ev.code, reason: ev.reason, wasClean: ev.wasClean });
        currentSocket = null;
        // Try reconnecting with exponential backoff up to a cap
        setTimeout(connectWebSocket, 2000);
    };
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

const createHostRow = (hostName: string): HTMLTableRowElement => {
    const template = (
        document.querySelector('#host-row-template') as HTMLTemplateElement
    ).content.firstElementChild!.cloneNode(true) as HTMLTableRowElement;
    template.dataset['hostname'] = hostName;

    template.querySelector('th')!
        .textContent = hostName;

    template.querySelector('.status')!
        .textContent = getHostStatus(hostName).statusText;

    template.querySelector('.leases')!
        .textContent = getFormattedLeases(hostName);

    const clientsConfigured = hasClientsConfigured();

    if (!clientsConfigured) template.querySelector('.leases')!.remove();

    const takeBtn = template.querySelector<HTMLButtonElement>('.take-lease')!;
    const releaseBtn = template.querySelector<HTMLButtonElement>('.release-lease')!;

    const takeText = clientsConfigured ? 'Take Lease' : 'Start';
    const releaseText = clientsConfigured ? 'Release Lease' : 'Shutdown';
    takeBtn.textContent = takeText;
    takeBtn.ariaLabel = takeText;
    releaseBtn.textContent = releaseText;
    releaseBtn.ariaLabel = releaseText;
    takeBtn.addEventListener('click', () => updateLease(hostName, 'take'));
    releaseBtn.addEventListener('click', () => updateLease(hostName, 'release'));

    return template;
};

const createClientRow = (clientId: string, leases: string[]): HTMLTableRowElement => {
    const template = (
        document.getElementById('client-row-template') as HTMLTemplateElement
    ).content.firstElementChild!.cloneNode(true) as HTMLTableRowElement;
    template.dataset['clientId'] = clientId;

    template.querySelector('th')!
        .textContent = clientId;

    template.querySelector<HTMLElement>('.leases')!
        .textContent = leases.join(', ') || 'None';

    const resetBtn = template.querySelector<HTMLButtonElement>('.reset-client')!;
    const resetText = 'Reset Leases';
    resetBtn.textContent = resetText;
    resetBtn.ariaLabel = resetText;
    if (leases.length === 0) resetBtn.disabled = true;
    resetBtn.addEventListener('click', () => resetClientLeases(clientId));

    return template;
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
    document.getElementById("host-table-leases-header")!.hidden = !hasClientsConfigured();
}

/**
 * Rebuild the hosts table from persisted host list and status map.
 * Active hosts are sorted alphabetically and displayed before inactive hosts.
 */
const updateHostsTable = () => {
    const hostTableBody = document.getElementById('host-table-body') as HTMLTableSectionElement;
    const hostList = sortActiveFirst(
        persistedHostsList,
        host => !!persistedStatusMap[host],
        host => host
    );
    hostTableBody.replaceChildren(...hostList.map(createHostRow));
    updateHostsTableHeader();
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

    const clientTableBody = document.getElementById('client-table-body') as HTMLTableSectionElement;
    clientTableBody.replaceChildren(...sortedClients.map(([clientId, leases]) => createClientRow(clientId, leases)));
};

// ==========================
// Backend Actions
// ==========================

/**
 * Send a lease action request to the backend.
 */
const updateLease = async (host: string, action: 'take' | 'release') => {
    if (DemoMode.isActive) {
        DemoMode.updateLease(host, action);
        return;
    }
    try {
        await fetch(`/api/lease/${host}/${action}`, { method: 'POST' });
    } catch (err) {
        console.error(`Failed to ${action} lease for ${host}:`, err);
    }
};

type UpdateLease = typeof updateLease;

/**
 * Request the backend to clear all leases owned by a given client.
 */
const resetClientLeases = async (clientId: string) => {
    if (DemoMode.isActive) {
        DemoMode.resetClientLeases(clientId);
        return;
    }
    try {
        await fetch(`/api/reset_leases/${clientId}`, { method: 'POST' });
    } catch (err) {
        console.error(`Failed to reset leases for client ${clientId}:`, err);
    }
};

type ResetClientLeases = typeof resetClientLeases;

const setupCopyButtons = () => {
    document.querySelectorAll<HTMLButtonElement>('.copy-button').forEach(button => {
        button.addEventListener('click', () => {
            const target = document.querySelector<HTMLElement>(button.dataset["copyTarget"] ?? "::ensureEmptyQuery")
                ?.textContent;
            if (target) {
                navigator.clipboard.writeText(target).then(() => {
                    button.textContent = "Copied!";
                    setTimeout(() => (button.textContent = "Copy"), 1500);
                });
            }
        });
    });
};

/**
 * Populate installer commands based on current origin.
 * This keeps embedded strings in the UI in sync with where the page is served from.
 */
const setupInstallerCommands = () => {
    const baseUrl = window.location.origin;

    // Install commands
    const hostInstallCommand = document.getElementById('host-install-command');
    const clientInstallCommandSh = document.getElementById('client-install-command-sh');
    const clientInstallCommandPs1 = document.getElementById('client-install-command-ps1');

    if (!hostInstallCommand || !clientInstallCommandSh || !clientInstallCommandPs1) {
        throw new Error('Missing required install command elements');
    }

    hostInstallCommand.textContent = `curl -fsSL ${baseUrl}/download/host_agent_installer.sh | sh -s ${baseUrl} --port 5757`;
    clientInstallCommandSh.textContent = `curl -sSL ${baseUrl}/download/client_installer.sh | sh -s ${baseUrl}`;
    clientInstallCommandPs1.textContent = `Invoke-WebRequest -Uri '${baseUrl}/download/client_installer.ps1' -OutFile 'client_installer.ps1'; powershell -ExecutionPolicy Bypass -File .\\client_installer.ps1 ${baseUrl}`;
}

// ==========================
// Initialization
// ==========================

document.addEventListener('DOMContentLoaded', () => {
    connectWebSocket();
    setupCopyButtons();
    setupInstallerCommands();
    if (DemoMode.isActive) {
        console.info('Demo mode enabled: UI is using simulated data.');
    }
});

// Handle Back-Forward Cache restoration
window.addEventListener('pageshow', (event) => {
    if (event.persisted) {
        console.info('Page restored from bfcache, reconnecting WebSocket');
        connectWebSocket();
    }
});

// Close WebSocket when page is hidden and will be cached
window.addEventListener('pagehide', (event) => {
    if (event.persisted && currentSocket) {
        console.info('Page being cached, closing WebSocket');
        currentSocket.close();
        currentSocket = null;
    }
});

/**
 * This code is made to run in Github Pages demo mode, to simulate a backend
 */
namespace DemoMode {
    /** Detected demo mode by presence of disclaimer element  */
    export const isActive = !!document.getElementById('demo-mode-disclaimer');

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

    export const updateLease: UpdateLease = async (host, action) => {
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

    export const resetClientLeases: ResetClientLeases = async (clientId) => {
        // For demo, just clear all leases for that client
        Object.keys(persistedLeaseMap).forEach(host => {
            persistedLeaseMap[host] = (persistedLeaseMap[host] || []).filter(l => l.type !== 'Client' || l.value !== clientId);
        });
        updateClientsTable();
        updateHostsTable();
    }
}
