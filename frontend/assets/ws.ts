import { applyMessage, type WsMessage } from './stores/appStore';

let currentSocket: WebSocket | null = null;

const showJSError = (message: string) => {
    const errorDiv = document.getElementById('js-error') as HTMLDivElement | null;
    const messageEl = document.getElementById('js-error-message') as HTMLParagraphElement | null;
    if (errorDiv && messageEl) {
        messageEl.textContent = message;
        errorDiv.hidden = false;
    }
};

const apiFetch = async (url: string, options?: RequestInit): Promise<Response> => {
    try {
        const resp = await fetch(url, options);
        if (resp.status === 401) {
            window.location.assign('/login');
            throw new Error('Unauthorized');
        }
        if (!resp.ok) {
            const msg = `HTTP ${resp.status}: ${resp.statusText}`;
            showJSError(msg);
            throw new Error(msg);
        }
        return resp;
    } catch (err) {
        if (!(err instanceof Error && err.message === 'Unauthorized')) {
            showJSError(err instanceof Error ? err.message : 'Unknown fetch error');
        }
        throw err;
    }
};

const checkAuthThenReconnect = async () => {
    try {
        await apiFetch('/api/hosts_status', { credentials: 'same-origin' });
    } catch (err) {
        if (err instanceof Error && err.message === 'Unauthorized') return;
        console.error('Auth check failed:', err);
    }
    setTimeout(connectWebSocket, 2000);
};

export const connectWebSocket = () => {
    if (currentSocket && currentSocket.readyState === WebSocket.OPEN) {
        console.info('WebSocket already connected');
        return;
    }
    if (currentSocket) currentSocket.close();

    const wsProtocol = location.protocol === 'https:' ? 'wss' : 'ws';
    const url = `${wsProtocol}://${location.host}/ws`;
    console.info('Attempting WebSocket connect to', url);
    const socket = new WebSocket(url);
    currentSocket = socket;

    socket.onopen = () => console.info('WebSocket connected to', url);
    socket.onmessage = (event: MessageEvent) => {
        try {
            applyMessage(JSON.parse(event.data) as WsMessage);
        } catch (err) {
            console.error('Error handling WS message:', err);
        }
    };
    socket.onerror = (ev) => console.error('WebSocket error', ev);
    socket.onclose = (ev) => {
        console.warn('WebSocket closed', { code: ev.code, reason: ev.reason, wasClean: ev.wasClean });
        currentSocket = null;
        checkAuthThenReconnect();
    };
};

export const closeWebSocket = () => {
    if (currentSocket) {
        currentSocket.close();
        currentSocket = null;
    }
};

// Back-Forward Cache handling
window.addEventListener('pageshow', (event) => {
    if (event.persisted) {
        console.info('Page restored from bfcache, reconnecting WebSocket');
        connectWebSocket();
    }
});

window.addEventListener('pagehide', (event) => {
    if (event.persisted && currentSocket) {
        console.info('Page being cached, closing WebSocket');
        currentSocket.close();
        currentSocket = null;
    }
});
