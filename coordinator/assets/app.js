let socket;

function connectWebSocket() {
    const wsProtocol = location.protocol === 'https:' ? 'wss' : 'ws';
    socket = new WebSocket(`${wsProtocol}://${location.host}/ws`);

    socket.onopen = () => console.log('WebSocket connected');
    socket.onmessage = handleWebSocketMessage;
    socket.onclose = () => setTimeout(connectWebSocket, 2000);
}

function handleWebSocketMessage(event) {
    try {
        const msg = event.data;
        msg === "config_updated" ? fetchNodes() : updateNodeStatuses(JSON.parse(msg));
    } catch (err) {
        console.error('Error parsing WS message:', err);
    }
}

function updateNodeStatuses(statusMap) {
    document.querySelectorAll('#host-table-body tr').forEach(row => {
        const hostname = row.dataset.hostname;
        const status = statusMap[hostname];
        if (status !== undefined) {
            row.querySelector('.status').textContent = status ? 'online' : 'offline';
            row.querySelector('.take-lease').disabled = status;
            row.querySelector('.release-lease').disabled = !status;
        }
    });
}

async function fetchNodes() {
    try {
        const response = await fetch('/api/nodes');
        const hosts = await response.json();
        document.getElementById('host-table-body').innerHTML = hosts.map(createHostRow).join('');
    } catch (err) {
        console.error('Failed to fetch hosts:', err);
    }
}

const createHostRow = host => `
    <tr data-hostname="${host.name}">
        <td>${host.name}</td>
        <td class="status">Loading...</td>
        <td>
            <button class="take-lease" onclick="updateLease('${host.name}', 'take')">Take Lease</button>
            <button class="release-lease shutdown" onclick="updateLease('${host.name}', 'release')">Release Lease</button>
        </td>
    </tr>
`;

async function updateLease(host, action) {
    try {
        await fetch(`/api/lease/${host}/${action}`, { method: 'POST' });
        setTimeout(fetchNodes, 1000);
    } catch (err) {
        console.error(`Failed to ${action} lease for ${host}:`, err);
    }
}

function setupCopyButtons() {
    document.querySelectorAll('.copy-button').forEach(button => {
        button.addEventListener('click', () => {
            const target = document.querySelector(button.dataset.copyTarget).textContent;
            navigator.clipboard.writeText(target).then(() => {
                button.textContent = "Copied!";
                setTimeout(() => button.textContent = "Copy", 1500);
            });
        });
    });
}

function initialize() {
    connectWebSocket();
    fetchNodes();
    setupCopyButtons();

    const baseUrl = window.location.origin;
    document.getElementById('install-command').textContent = `curl -fsSL ${baseUrl}/download/node_agent_installer.sh | sh -s ${baseUrl} --port 9090`;
    document.getElementById('client-script-url').textContent = `${baseUrl}/download/shuthost_client.sh?remote_url=${encodeURIComponent(baseUrl)}`;
}

document.addEventListener('DOMContentLoaded', initialize);