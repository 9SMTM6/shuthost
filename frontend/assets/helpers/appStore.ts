import { createStore, produce } from 'solid-js/store';
import { serverData } from './serverData';

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

export type HostStats = {
    lastOnline: string | null;
};

export type DbData = {
    clientStats: Record<string, ClientStats>;
    hostStats: Record<string, HostStats>;
};

export type AppState = {
    hosts: string[];
    statusMap: StatusMap;
    leaseMap: Record<string, LeaseSource[]>;
    clients: string[];
    dbData: DbData | null;
};

// TODO:
// * Add updates for client stats and (upcoming) host stats, IIRC I went against that in the past cause it was annoying to do in raw html + js without signals etc.
export type WsMessage =
    | { type: 'HostStatus'; payload: StatusMap }
    | { type: 'ConfigChanged'; payload: { hosts: string[]; clients: string[] } }
    | {
          type: 'Initial';
          payload: AppState;
      }
    | { type: 'LeaseUpdate'; payload: { host: string; leases: LeaseSource[] } };

// ==========================
// Store
// ==========================

const [state, setState] = createStore<AppState>({
    hosts: [],
    statusMap: {},
    leaseMap: {},
    clients: [],
    dbData: null,
});

export { state };

export const hasDb = (s: AppState): s is AppState & { dbData: DbData } => s.dbData !== null;

export const applyMessage = (message: WsMessage) => {
    switch (message.type) {
        case 'Initial':
            if ((message.payload.dbData !== null) !== serverData.dbEnabled) {
                throw new Error(
                    `serverData.dbEnabled (${serverData.dbEnabled}) disagrees with received dbData (${message.payload.dbData === null ? 'null' : 'present'})`,
                );
            }
            setState(message.payload);
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
