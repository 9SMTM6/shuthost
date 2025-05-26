let socket;

function connectWebSocket() {
    const wsProtocol = location.protocol === 'https:' ? 'wss' : 'ws';
    const socket = new WebSocket(`${wsProtocol}://${location.host}/ws`);

    socket.onopen = () => {
        console.log('WebSocket connected');
    };

    socket.onmessage = (event) => {
        const msg = event.data;
        try {
            if (msg === "config_updated") {
                fetchNodes();
            } else {
                const statusMap = JSON.parse(msg);
                updateNodeStatuses(statusMap);
            }
        } catch (err) {
            console.error('Error parsing WS message:', err);
        }
    };

    socket.onclose = () => {
        console.warn('WebSocket closed, reconnecting in 2s...');
        setTimeout(connectWebSocket, 2000);
    };
}

function updateNodeStatuses(statusMap) {
    const tbody = document.getElementById('host-table-body');
    for (const row of tbody.rows) {
        const hostname = row.cells[0].textContent;
        const status = statusMap[hostname];
        if (status !== undefined) {
            row.cells[1].textContent = status ? 'online' : 'offline';
            const wakeBtn = row.cells[2].children[0];
            const shutdownBtn = row.cells[2].children[1];
            wakeBtn.disabled = status;
            shutdownBtn.disabled = !status;
        }
    }
}

async function fetchNodes() {
    try {
        const response = await fetch('/api/nodes');
        const hosts = await response.json();
        const tbody = document.getElementById('host-table-body');
        tbody.innerHTML = '';

        hosts.forEach(async (host) => {
            const row = document.createElement('tr');
            const nameCell = document.createElement('td');
            nameCell.textContent = host.name;
            row.appendChild(nameCell);

            const statusCell = document.createElement('td');
            const status = await fetchNodeStatus(host.name);
            statusCell.textContent = status;
            row.appendChild(statusCell);

            const actionsCell = document.createElement('td');
            const startBtn = document.createElement('button');
            startBtn.textContent = 'Take Lease';
            startBtn.disabled = status === 'online';
            const startIndicator = createLoadingIndicator();
            startBtn.appendChild(startIndicator);
            startBtn.onclick = () => sendLeaseUpdate(host.name, 'take', startIndicator);

            const shutdownBtn = document.createElement('button');
            shutdownBtn.textContent = 'Release Lease';
            shutdownBtn.className = 'shutdown';
            shutdownBtn.disabled = status === 'offline';
            const shutdownIndicator = createLoadingIndicator();
            shutdownBtn.appendChild(shutdownIndicator);
            shutdownBtn.onclick = () => sendLeaseUpdate(host.name, 'release', shutdownIndicator);

            actionsCell.appendChild(startBtn);
            actionsCell.appendChild(shutdownBtn);
            row.appendChild(actionsCell);

            tbody.appendChild(row);
        });
    } catch (err) {
        console.error('Failed to fetch hosts:', err);
    }
}

async function fetchNodeStatus(hostname) {
    try {
        const response = await fetch(`/api/status/${hostname}`);
        return await response.text();
    } catch (err) {
        console.error(`Failed to fetch status for ${hostname}:`, err);
        return 'offline';
    }
}

function createLoadingIndicator() {
    const indicator = document.createElement('div');
    indicator.classList.add('loading-indicator');
    indicator.style.display = 'none';
    return indicator;
}

async function sendLeaseUpdate(host, leaseAction, indicator) {
    try {
        indicator.style.display = 'inline-block';
        await fetch(`/api/lease/${host}/${leaseAction}`, { method: 'POST' });
        setTimeout(fetchNodes, 1000);
    } catch (err) {
        console.error(`Failed to ${leaseAction} lease on host ${host}:`, err);
    } finally {
        indicator.style.display = 'none';
    }
}

function copyToClipboard() {
    const command = document.getElementById('install-command').textContent;
    navigator.clipboard.writeText(command).then(() => {
        const button = document.querySelector('.copy-button');
        const original = button.textContent;
        button.textContent = "Copied!";
        setTimeout(() => {
            button.textContent = original;
        }, 1500);
    });
}

function updateClientScriptUrl() {
    const href = window.location.origin;
    const url = `${href}/download/shuthost_client.sh?remote_url=${encodeURIComponent(href)}`;
    document.getElementById('client-script-url').textContent = url;
}

function copyClientScriptUrl() {
    const url = document.getElementById('client-script-url').textContent;
    navigator.clipboard.writeText(url).then(() => {
        const button = document.querySelector('.code-block:nth-of-type(2) .copy-button');
        const original = button.textContent;
        button.textContent = "Copied!";
        setTimeout(() => {
            button.textContent = original;
        }, 1500);
    });
}

// Initialize
document.addEventListener('DOMContentLoaded', () => {
    connectWebSocket();
    fetchNodes();
    updateClientScriptUrl();
    
    const href = window.location.origin;
    const command = `curl -fsSL ${href}/download/node_agent_installer.sh | sh -s ${href} --port 9090`;
    document.getElementById('install-command').textContent = command;
});