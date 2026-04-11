import { createStore, produce } from 'solid-js/store';
import { serverData } from './serverData';
import { Infer, is } from './assertData';

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

export const hostStatChecker = is.object({
    lastOnline: is.optional(is.string),
    agentVersion: is.optional(is.string),
    initSystem: is.optional(is.oneOf('systemd', 'openrc', 'self-extracting-shell', 'self-extracting-pwsh', 'launchd')),
    operatingSystem: is.optional(is.oneOf('windows', 'linux', 'macos')),
    scriptPath: is.optional(is.string),
    isOnline: is.optional(is.boolean),
} as const);

export type HostStats = Infer<typeof hostStatChecker>;

export type DbData = {
    clientStats: Record<string, ClientStats>;
    hostStats: Record<string, HostStats>;
};

export type DbDataState =
    | { status: 'disabled' }
    | { status: 'available'; payload: DbData }
    | { status: 'error'; payload: { message: string } };

/** The configuration values (mapped to what the frontend can see) that the coordinator accepts runtime changes of */
type DynamicConfig = {
    hosts: string[];
    clients: string[];
};

export type AppState = {
    statusMap: StatusMap;
    leaseMap: Record<string, LeaseSource[]>;
    dbData: DbDataState;
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
    dbData: { status: 'disabled' },
});

export { state };

export const hasDb = (
    s: AppState,
): s is AppState & { dbData: { status: 'available'; payload: DbData } } =>
    s.dbData.status === 'available';

export const applyMessage = (message: WsMessage) => {
    switch (message.type) {
        case 'Initial': {
            const isDisabled = message.payload.dbData.status === 'disabled';
            if (!serverData.dbEnabled && !isDisabled) {
                throw new Error(
                    `serverData.dbEnabled (${serverData.dbEnabled}) disagrees with received dbData status (${message.payload.dbData.status})`,
                );
            }
            setState(message.payload);
            break;
        }
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
            throw new Error(
                `Unhandled WebSocket message: ${JSON.stringify(_exhaustive)}`,
            );
        }
    }
};
