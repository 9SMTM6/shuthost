// Reusable types
type Host = { name: string };
type StatusMap = Record<string, boolean>;

// Persist statusMap globally
let statusMap: StatusMap = {};

const connectWebSocket = () => {
    const wsProtocol = location.protocol === 'https:' ? 'wss' : 'ws';
    const socket = new WebSocket(`${wsProtocol}://${location.host}/ws`);

    socket.onopen = () => console.log('WebSocket connected');
    socket.onmessage = handleWebSocketMessage;
    socket.onclose = () => setTimeout(connectWebSocket, 2000);
};

const handleWebSocketMessage = (event: MessageEvent) => {
    try {
        const msg = event.data;
        if (msg === "config_updated") {
            fetchNodes();
        } else {
            // Update and persist statusMap
            statusMap = JSON.parse(msg) as StatusMap;
            updateNodeStatuses(statusMap);
        }
    } catch (err) {
        console.error('Error parsing WS message:', err);
    }
};

const updateNodeStatuses = (statusMap: StatusMap) => {
    document.querySelectorAll<HTMLTableRowElement>('#host-table-body tr').forEach(row => {
        const hostname = row.dataset["hostname"];
        if (hostname) {
            const status = statusMap[hostname];
            if (status !== undefined) {
                const statusCell = row.querySelector<HTMLElement>('.status');
                const takeLeaseButton = row.querySelector<HTMLButtonElement>('.take-lease');
                const releaseLeaseButton = row.querySelector<HTMLButtonElement>('.release-lease');

                if (statusCell) statusCell.textContent = status ? 'online' : 'offline';
                if (takeLeaseButton) takeLeaseButton.disabled = status;
                if (releaseLeaseButton) releaseLeaseButton.disabled = !status;
            }
        }
    });
};

const fetchNodes = async () => {
    try {
        const response = await fetch('/api/nodes');
        const hosts: Host[] = await response.json();
        const hostTableBody = document.getElementById('host-table-body');
        if (hostTableBody) {
            hostTableBody.innerHTML = hosts.map(createHostRow).join('');
            // After re-render, re-apply statuses if available
            if (Object.keys(statusMap).length > 0) {
                updateNodeStatuses(statusMap);
            }
        }
    } catch (err) {
        console.error('Failed to fetch hosts:', err);
    }
};

const createHostRow = (host: Host) => `
    <tr data-hostname="${host.name}" class="hover:bg-gray-50">
        <td class="table-header border-none">${host.name}</td>
        <td class="table-header border-none status">Loading...</td>
        <td class="table-header border-none flex flex-col sm:flex-row gap-2 sm:gap-4">
            <button class="btn btn-green take-lease" onclick="updateLease('${host.name}', 'take')">Take Lease</button>
            <button class="btn btn-red release-lease" onclick="updateLease('${host.name}', 'release')">Release Lease</button>
        </td>
    </tr>
`;

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

// Add helper function for inserting break opportunities
const addBreakOpportunities = (text: string) => {
    return text.replace(/([/._-])/g, `$1\u200B`);
};

const initialize = () => {
    connectWebSocket();
    fetchNodes();
    setupCopyButtons();

    const baseUrl = window.location.origin;
    const nodeInstallCommand = document.getElementById('node-install-command');
    const clientInstallCommand = document.getElementById('client-install-command');

    if (nodeInstallCommand) {
        nodeInstallCommand.textContent = `curl -fsSL ${addBreakOpportunities(`${baseUrl}/download/node_agent_installer.sh`)} | sh -s ${addBreakOpportunities(baseUrl)} --port 5757`;
    }

    if (clientInstallCommand) {
        clientInstallCommand.textContent = `curl -fsSL ${addBreakOpportunities(`${baseUrl}/download/client_installer.sh`)} | sh -s ${addBreakOpportunities(baseUrl)}`;
    }
};

document.addEventListener('DOMContentLoaded', initialize);