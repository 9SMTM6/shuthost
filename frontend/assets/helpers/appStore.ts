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
    lastUsed: string | null;
};

export type HostStats = {
    lastOnline: string | null;
    isOnline?: boolean;
};

export type DbData = {
    clientStats: Record<string, ClientStats>;
    hostStats: Record<string, HostStats>;
};

/** The configuration values (mapped to what the frontend can see) that the coordinator accepts runtime changes of */
type DynamicConfig = {
    hosts: string[];
    clients: string[];
};

export type AppState = {
    statusMap: StatusMap;
    leaseMap: Record<string, LeaseSource[]>;
    dbData: DbData | null;
} & DynamicConfig;

// TODO:
// * Add updates for client stats and (upcoming) host stats, IIRC I went against that in the past cause it was annoying to do in raw html + js without signals etc.
export type WsMessage =
    | { type: 'HostStatus'; payload: StatusMap }
    | { type: 'ConfigChanged'; payload: DynamicConfig }
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
        default: {
            const _exhaustive: never = message;
            throw new Error(`Unhandled WebSocket message: ${JSON.stringify(_exhaustive)}`);
        }
    }
};
