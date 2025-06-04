let socket: WebSocket | undefined;

// TODO: consider how to properly integrate this with the rest of the app (in terms of compilation and bundling)

function connectWebSocket(): void {
    const wsProtocol = location.protocol === 'https:' ? 'wss' : 'ws';
    socket = new WebSocket(`${wsProtocol}://${location.host}/ws`);

    socket.onopen = () => console.log('WebSocket connected');
    socket.onmessage = handleWebSocketMessage;
    socket.onclose = () => setTimeout(connectWebSocket, 2000);
}

function handleWebSocketMessage(event: MessageEvent): void {
    try {
        const msg = event.data;
        if (msg === "config_updated") {
            fetchNodes();
        } else {
            updateNodeStatuses(JSON.parse(msg) as Record<string, boolean>);
        }
    } catch (err) {
        console.error('Error parsing WS message:', err);
    }
}

function updateNodeStatuses(statusMap: Record<string, boolean>): void {
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
}

async function fetchNodes(): Promise<void> {
    try {
        const response = await fetch('/api/nodes');
        const hosts: { name: string }[] = await response.json();
        const hostTableBody = document.getElementById('host-table-body');
        if (hostTableBody) {
            hostTableBody.innerHTML = hosts.map(createHostRow).join('');
        }
    } catch (err) {
        console.error('Failed to fetch hosts:', err);
    }
}

const createHostRow = (host: { name: string }): string => `
    <tr data-hostname="${host.name}">
        <td>${host.name}</td>
        <td class="status">Loading...</td>
        <td>
            <button class="take-lease" onclick="updateLease('${host.name}', 'take')">Take Lease</button>
            <button class="release-lease shutdown" onclick="updateLease('${host.name}', 'release')">Release Lease</button>
        </td>
    </tr>
`;

async function updateLease(host: string, action: string): Promise<void> {
    try {
        await fetch(`/api/lease/${host}/${action}`, { method: 'POST' });
    } catch (err) {
        console.error(`Failed to ${action} lease for ${host}:`, err);
    }
}

function setupCopyButtons(): void {
    document.querySelectorAll<HTMLButtonElement>('.copy-button').forEach(button => {
        button.addEventListener('click', () => {
            const targetSelector = button.dataset["copyTarget"];
            if (targetSelector) {
                const target = document.querySelector<HTMLElement>(targetSelector)?.textContent;
                if (target) {
                    navigator.clipboard.writeText(target).then(() => {
                        button.textContent = "Copied!";
                        setTimeout(() => button.textContent = "Copy", 1500);
                    });
                }
            }
        });
    });
}

function initialize(): void {
    connectWebSocket();
    fetchNodes();
    setupCopyButtons();

    const baseUrl = window.location.origin;
    const nodeInstallCommand = document.getElementById('node-install-command');
    const clientInstallCommand = document.getElementById('client-install-command');

    if (nodeInstallCommand) {
        nodeInstallCommand.textContent = `curl -fsSL ${baseUrl}/download/node_agent_installer.sh | sh -s ${baseUrl} --port 5757`;
    }

    if (clientInstallCommand) {
        clientInstallCommand.textContent = `curl -fsSL ${baseUrl}/download/client_installer.sh | sh -s ${baseUrl}`;
    }
}

document.addEventListener('DOMContentLoaded', initialize);