// Reusable types
type Host = { name: string };
type StatusMap = Record<string, boolean>;

type LeaseSource =
    | { type: 'WebInterface' }
    | { type: 'Client'; value: string };

type WsMessage = 
    | { type: 'HostStatus'; payload: Record<string, boolean> }
    | { type: 'UpdateNodes'; payload: string[] }
    | { type: 'Initial'; payload: { nodes: string[]; status: Record<string, boolean>; leases: Record<string, LeaseSource[]> } }
    | { type: 'LeaseUpdate'; payload: { node: string; leases: LeaseSource[] } };

type Client = {
    id: string;
    leases: string[];
};


// Persist statusMap globally
let persistedStatusMap: StatusMap = {};
let persistedLeaseMap: Record<string, LeaseSource[]> = {};

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
        
        switch (message.type) {
            case 'Initial':
                persistedStatusMap = message.payload.status;
                persistedLeaseMap = message.payload.leases;
                const hosts = message.payload.nodes.map(name => ({ name }));
                hostTableBody.innerHTML = hosts.map(createHostRow).join('');
                updateClientsTable(); // Add this line
                break;
            case 'HostStatus':
                persistedStatusMap = message.payload;
                break;
            case 'UpdateNodes':
                const newHosts = message.payload.map(name => ({ name }));
                hostTableBody.innerHTML = newHosts.map(createHostRow).join('');
                break;
            case 'LeaseUpdate':
                const { node, leases } = message.payload;
                persistedLeaseMap[node] = leases;
                updateClientsTable(); // Add this line
                console.log(`Updated leases for ${node}:`, persistedLeaseMap[node]);
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
    console.log(`Formatting leases for ${hostname}:`, persistedLeaseMap[hostname]);
    return persistedLeaseMap[hostname]?.map(formatLeaseSource).join(', ') || 'None';
};

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
    return `
    <tr data-hostname="${host.name}" class="hover:bg-gray-50">
        <td class="table-cell">${host.name}</td>
        <td class="table-cell status">${statusText}</td>
        <td class="table-cell leases">${leases}</td>
        <td class="table-cell">
            <div class="actions-cell">
                <button class="btn btn-green take-lease" onclick="updateLease('${host.name}', 'take')">Take Lease</button>
                <button class="btn btn-red release-lease" onclick="updateLease('${host.name}', 'release')">Release Lease</button>
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

const initialize = () => {
    const nodeInstallCommand = document.getElementById('node-install-command');
    const clientInstallCommand = document.getElementById('client-install-command');
    
    if (!nodeInstallCommand) throw new Error('Missing required element #node-install-command');
    if (!clientInstallCommand) throw new Error('Missing required element #client-install-command');

    connectWebSocket();
    setupCopyButtons();
    setupTabs();

    const baseUrl = window.location.origin;
    nodeInstallCommand.textContent = `curl -fsSL ${`${baseUrl}/download/node_agent_installer.sh`} | sh -s ${baseUrl} --port 5757`;
    clientInstallCommand.textContent = `curl -fsSL ${`${baseUrl}/download/client_installer.sh`} | sh -s ${baseUrl}`;
};

document.addEventListener('DOMContentLoaded', initialize);

const formatLeaseSource = (lease: LeaseSource): string => {
    switch (lease.type) {
        case 'WebInterface':
            return 'web-interface';
        case 'Client':
            return `client-${lease.value}`;
    }
};

// Add this function to create client table rows
const createClientRow = (clientId: string, leases: string[]) => {
    const hasLeases = leases.length > 0;
    return `
    <tr data-client-id="${clientId}" class="hover:bg-gray-50">
        <td class="table-cell">client-${clientId}</td>
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

// Add this function to update the clients table
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

    const clientTableBody = document.getElementById('client-table-body');
    if (clientTableBody) {
        clientTableBody.innerHTML = Array.from(clientMap.entries())
            .map(([clientId, leases]) => createClientRow(clientId, leases))
            .join('');
    }
};

// Add this function to handle resetting client leases
const resetClientLeases = async (clientId: string) => {
    try {
        await fetch(`/api/client/${clientId}/reset-leases`, { method: 'POST' });
    } catch (err) {
        console.error(`Failed to reset leases for client ${clientId}:`, err);
    }
};