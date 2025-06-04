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
    // TODO: add endpoint to get leases, and enable/disable buttons based on lease status.
    // Ensure order of hosts is stable (online hosts in the beginning).
    // add a "lease" column to the table, showing lease status.
    // Add separate table to show configured clients, with a reset button to fix broken leases.
    // Then add rework wording/UI of GUI leases to be understandable without understanding leases (if someone doesnt need them).
    // Then add a bunch of documentation to explain:
    // - how to configure WOL and gotchas
    // - shuthost architecture
    // - how leases work
    // Then rework UI to be css-grid based, and test on mobile.
    // then consider different global layout (with tabs?)
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
    document.getElementById('node-install-command').textContent = `curl -fsSL ${baseUrl}/download/node_agent_installer.sh | sh -s ${baseUrl} --port 5757`;
    document.getElementById('client-install-command').textContent = `curl -fsSL ${baseUrl}/download/client_installer.sh | sh -s ${baseUrl}`;
}

document.addEventListener('DOMContentLoaded', initialize);