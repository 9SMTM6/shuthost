// Reusable types
type Host = { name: string };
type StatusMap = Record<string, boolean>;

type WsMessage = 
    | { type: 'HostStatus'; payload: Record<string, boolean> }
    | { type: 'UpdateNodes'; payload: string[] }
    | { type: 'Initial'; payload: { nodes: string[]; status: Record<string, boolean> } };

// Persist statusMap globally
let persistedStatusMap: StatusMap = {};

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
                const hosts = message.payload.nodes.map(name => ({ name }));
                hostTableBody.innerHTML = hosts.map(it => createHostRow(it, persistedStatusMap)).join('');
                break;
            case 'HostStatus':
                persistedStatusMap = message.payload;
                updateNodeStatuses(persistedStatusMap);
                break;
            case 'UpdateNodes':
                const newHosts = message.payload.map(name => ({ name }));
                hostTableBody.innerHTML = newHosts.map(it => createHostRow(it, persistedStatusMap)).join('');
                break;
        }
    } catch (err) {
        console.error('Error handling WS message:', err);
        throw err; // Re-throw to make the error more visible
    }
};

const getHostStatus = (hostname: string, statusMap: StatusMap) => {
    const status = statusMap[hostname];
    return {
        statusText: status === undefined ? 'Loading...' : (status ? 'online' : 'offline'),
        takeLeaseDisabled: status ? 'disabled' : '',
        releaseLeaseDisabled: !status ? 'disabled' : ''
    };
};

const createHostRow = (host: Host, statusMap: StatusMap) => {
    const { statusText, takeLeaseDisabled, releaseLeaseDisabled } = getHostStatus(host.name, statusMap);
    return `
    <tr data-hostname="${host.name}" class="hover:bg-gray-50">
        <td class="table-header border-none">${host.name}</td>
        <td class="table-header border-none status">${statusText}</td>
        <td class="table-header border-none flex flex-col sm:flex-row gap-2 sm:gap-4">
            <button class="btn btn-green take-lease" onclick="updateLease('${host.name}', 'take')" ${takeLeaseDisabled}>Take Lease</button>
            <button class="btn btn-red release-lease" onclick="updateLease('${host.name}', 'release')" ${releaseLeaseDisabled}>Release Lease</button>
        </td>
    </tr>
`;
};

const updateNodeStatuses = (statusMap: StatusMap) => {
    document.querySelectorAll<HTMLTableRowElement>('#host-table-body tr').forEach(row => {
        const hostname = row.dataset["hostname"];
        if (hostname) {
            const { statusText, takeLeaseDisabled, releaseLeaseDisabled } = getHostStatus(hostname, statusMap);
            const statusCell = row.querySelector<HTMLElement>('.status');
            const takeLeaseButton = row.querySelector<HTMLButtonElement>('.take-lease');
            const releaseLeaseButton = row.querySelector<HTMLButtonElement>('.release-lease');

            if (statusCell) statusCell.textContent = statusText;
            if (takeLeaseButton) takeLeaseButton.disabled = !!takeLeaseDisabled;
            if (releaseLeaseButton) releaseLeaseButton.disabled = !!releaseLeaseDisabled;
        }
    });
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

// Add helper function for inserting break opportunities
const addBreakOpportunities = (text: string) => {
    return text.replace(/([/._-])/g, `$1\u200B`);
};

const initialize = () => {
    const nodeInstallCommand = document.getElementById('node-install-command');
    const clientInstallCommand = document.getElementById('client-install-command');
    
    if (!nodeInstallCommand) throw new Error('Missing required element #node-install-command');
    if (!clientInstallCommand) throw new Error('Missing required element #client-install-command');

    connectWebSocket();
    setupCopyButtons();

    const baseUrl = window.location.origin;
    nodeInstallCommand.textContent = `curl -fsSL ${addBreakOpportunities(`${baseUrl}/download/node_agent_installer.sh`)} | sh -s ${addBreakOpportunities(baseUrl)} --port 5757`;
    clientInstallCommand.textContent = `curl -fsSL ${addBreakOpportunities(`${baseUrl}/download/client_installer.sh`)} | sh -s ${addBreakOpportunities(baseUrl)}`;
};

document.addEventListener('DOMContentLoaded', initialize);