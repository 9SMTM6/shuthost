import { createStore, produce } from 'solid-js/store';

// ==========================
// Types
// ==========================

export type StatusMap = Record<string, 'online' | 'offline'>;

export type LeaseSource =
    | { type: 'WebInterface' }
    | { type: 'Client'; value: string };

export type ClientStats = {
    last_used: string | null;
};

export type WsMessage =
    | { type: 'HostStatus'; payload: StatusMap }
    | { type: 'ConfigChanged'; payload: { hosts: string[]; clients: string[] } }
    | { type: 'Initial'; payload: { hosts: string[]; clients: string[]; status: StatusMap; leases: Record<string, LeaseSource[]>; client_stats: Record<string, ClientStats> | null; broadcast_port: number } }
    | { type: 'LeaseUpdate'; payload: { host: string; leases: LeaseSource[] } };

export type AppState = {
    hosts: string[];
    statusMap: StatusMap;
    leaseMap: Record<string, LeaseSource[]>;
    clients: string[];
    clientStats: Record<string, ClientStats> | null;
    broadcastPort: number | undefined;
};

// ==========================
// Store
// ==========================

const [state, setState] = createStore<AppState>({
    hosts: [],
    statusMap: {},
    leaseMap: {},
    clients: [],
    clientStats: null,
    broadcastPort: undefined,
});

export { state };

export const applyMessage = (message: WsMessage) => {
    switch (message.type) {
        case 'Initial':
            setState({
                hosts: message.payload.hosts,
                clients: message.payload.clients,
                statusMap: message.payload.status,
                leaseMap: message.payload.leases,
                clientStats: message.payload.client_stats,
                broadcastPort: message.payload.broadcast_port,
            });
            break;
        case 'HostStatus':
            setState('statusMap', message.payload);
            break;
        case 'ConfigChanged':
            setState('hosts', message.payload.hosts);
            setState('clients', message.payload.clients);
            break;
        case 'LeaseUpdate':
            setState(produce((s) => {
                s.leaseMap[message.payload.host] = message.payload.leases;
            }));
            break;
    }
};
