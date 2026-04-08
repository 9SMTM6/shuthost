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

// TODO:
// * explore defining this depending on the de presence, perhaps add db being enabled to serverdata json and use that to have globally dynamically correct typing
// * Add updates for client stats and (upcoming) host stats, IIRC I went against that in the past cause it was annoying to do in raw html + js without signals etc.
export type WsMessage =
    | { type: 'HostStatus'; payload: StatusMap }
    | { type: 'ConfigChanged'; payload: { hosts: string[]; clients: string[] } }
    | {
          type: 'Initial';
          payload: {
              hosts: string[];
              clients: string[];
              status: StatusMap;
              leases: Record<string, LeaseSource[]>;
              client_stats: Record<string, ClientStats> | null;
              host_last_online: Record<string, string> | null;
          };
      }
    | { type: 'LeaseUpdate'; payload: { host: string; leases: LeaseSource[] } };

export type AppState = {
    hosts: string[];
    statusMap: StatusMap;
    leaseMap: Record<string, LeaseSource[]>;
    clients: string[];
    clientStats: Record<string, ClientStats> | null;
    hostLastOnline: Record<string, string> | null;
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
    hostLastOnline: null,
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
                hostLastOnline: message.payload.host_last_online,
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
            setState(
                produce((s) => {
                    s.leaseMap[message.payload.host] = message.payload.leases;
                }),
            );
            break;
    }
};
