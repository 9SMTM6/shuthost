import { applyMessage } from '../appStore';

let currentSocket: WebSocket | null = null;
let _heartbeatIntervalId: number | null = null;
let _heartbeatTimeoutId: number | null = null;
const HEARTBEAT_INTERVAL_MS = 30000; // send ping every 30s
const HEARTBEAT_TIMEOUT_MS = 10000; // wait 10s for pong

const checkAuthThenReconnect = async () => {
    try {
        const resp = await fetch('/api/hosts_status', {
            method: 'HEAD',
            credentials: 'same-origin',
        });
        if (resp.status === 401) {
            console.warn(
                `Auth probe: received ${resp.status} (expected for unauthenticated users)`,
            );
            window.location.assign('/login');
            return;
        }
        if (!resp.ok) {
            console.error('Auth check failed:', `HTTP ${resp.status}`);
        }
    } catch (err) {
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
        let parsed: unknown;
        try {
            parsed = JSON.parse(event.data);
        } catch {
            parsed = null;
        }

        try {
            // Try to parse app-level control frames (ping/pong) first.
            if (parsed && typeof parsed === 'object' && 'type' in parsed) {
                const t = parsed.type;
                if (t === 'pong') {
                    // Received heartbeat response from server.
                    if (_heartbeatTimeoutId) {
                        clearTimeout(_heartbeatTimeoutId);
                        _heartbeatTimeoutId = null;
                    }
                    return;
                }
            }

            applyMessage(parsed);
        } catch (err) {
            console.error('Error handling WS message:', err);
            setTimeout(() => {
                throw err;
            });
        }
    };
    socket.onerror = (ev) => console.error('WebSocket error', ev);
    socket.onclose = (ev) => {
        console.warn('WebSocket closed', {
            code: ev.code,
            reason: ev.reason,
            wasClean: ev.wasClean,
        });
        currentSocket = null;
        // Clear heartbeat timers
        if (_heartbeatIntervalId) {
            clearInterval(_heartbeatIntervalId);
            _heartbeatIntervalId = null;
        }
        if (_heartbeatTimeoutId) {
            clearTimeout(_heartbeatTimeoutId);
            _heartbeatTimeoutId = null;
        }
        checkAuthThenReconnect();
    };

    // Start application-level heartbeat: send ping periodically and expect a pong.
    // Use a small delay to avoid racing open events in quick reconnect loops.
    if (_heartbeatIntervalId) {
        clearInterval(_heartbeatIntervalId);
        _heartbeatIntervalId = null;
    }
    _heartbeatIntervalId = window.setInterval(() => {
        if (!currentSocket || currentSocket.readyState !== WebSocket.OPEN)
            return;
        try {
            currentSocket.send(
                JSON.stringify({ type: 'ping', ts: Date.now() }),
            );
            // Set a timeout; if no pong arrives in time, force reconnect.
            if (_heartbeatTimeoutId) clearTimeout(_heartbeatTimeoutId);
            _heartbeatTimeoutId = window.setTimeout(() => {
                console.warn(
                    'Heartbeat timeout — closing socket to trigger reconnect',
                );
                try {
                    currentSocket?.close();
                } catch {}
            }, HEARTBEAT_TIMEOUT_MS);
        } catch (e) {
            console.error('Failed sending heartbeat ping', e);
        }
    }, HEARTBEAT_INTERVAL_MS);
};

export const closeWebSocket = () => {
    if (currentSocket) {
        currentSocket.close();
        currentSocket = null;
    }
};
