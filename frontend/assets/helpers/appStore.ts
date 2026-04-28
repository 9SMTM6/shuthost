import { createStore, produce } from 'solid-js/store';
import { type Infer, is } from './assertData';
import { serverData } from './serverData';

// ==========================
// Types
// ==========================

export const statusOptions = [
    'online',
    'offline',
    'waking',
    'shutting_down',
] as const;
export type Status = (typeof statusOptions)[number];
export type StatusMap = Record<string, Status>;

export type LeaseSource =
    | { type: 'WebInterface' }
    | { type: 'Client'; value: string };

export type ClientStats = {
    lastUsed: string | null;
};

export const hostStatChecker = is.object({
    lastOnline: is.optional(is.string),
    agentVersion: is.optional(is.string),
    initSystem: is.optional(
        is.oneOf(
            'systemd',
            'openrc',
            'self-extracting-shell',
            'self-extracting-pwsh',
            'launchd',
        ),
    ),
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
    operationFailures: Record<string, OperationFailure>;
} & DynamicConfig;

export type OperationFailure = {
    operation: 'shutdown' | 'startup';
};

// TODO:
// * Add updates for client stats and (upcoming) host stats, IIRC I went against that in the past cause it was annoying to do in raw html + js without signals etc.
export type WsMessage =
    | { type: 'HostStatus'; payload: StatusMap }
    | { type: 'ClientStats'; payload: Record<string, ClientStats> }
    | { type: 'HostStats'; payload: { host: string; stats: HostStats } }
    | { type: 'ConfigChanged'; payload: DynamicConfig }
    | {
          type: 'Initial';
          payload: AppState;
      }
    | { type: 'LeaseUpdate'; payload: { host: string; leases: LeaseSource[] } }
    | { type: 'OperationFailed'; payload: Record<string, OperationFailure> };

// ==========================
// Store
// ==========================

const [state, setState] = createStore<AppState>({
    hosts: [],
    statusMap: {},
    leaseMap: {},
    clients: [],
    dbData: { status: 'disabled' },
    operationFailures: {},
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
        case 'ClientStats':
            setState(
                produce((s: AppState) => {
                    if (s.dbData.status === 'available') {
                        for (const [clientId, stats] of Object.entries(
                            message.payload,
                        )) {
                            s.dbData.payload.clientStats[clientId] = stats;
                        }
                    }
                }),
            );
            break;
        case 'HostStats':
            setState(
                produce((s: AppState) => {
                    if (s.dbData.status === 'available') {
                        s.dbData.payload.hostStats[message.payload.host] =
                            message.payload.stats;
                    }
                }),
            );
            break;
        case 'OperationFailed':
            setState('operationFailures', message.payload);
            break;
        default: {
            const _exhaustive: never = message;
            throw new Error(
                `Unhandled WebSocket message: ${JSON.stringify(_exhaustive)}`,
            );
        }
    }
};
