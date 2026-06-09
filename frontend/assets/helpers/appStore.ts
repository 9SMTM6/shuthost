import { createStore, produce } from 'solid-js/store';
import { type Infer, is, validateDataAsync } from './assertData';
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

const statusOptionsChecker = is.oneOf(...statusOptions);
export type Status = Infer<typeof statusOptionsChecker>;

const statusMapChecker = is.recordOf(statusOptionsChecker);

const clientLeaseChecker = is.object({
    type: 'Client',
    value: is.string,
} as const);

export type ClientLease = Infer<typeof clientLeaseChecker>;

const leaseSourceChecker = is.oneOf(
    is.object({ type: 'WebInterface' } as const),
    clientLeaseChecker,
);

export type LeaseSource = Infer<typeof leaseSourceChecker>;

const clientStatsChecker = is.object({
    lastUsed: is.optional(is.string),
} as const);
export type ClientStats = Infer<typeof clientStatsChecker>;

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

const hostHookActionChecker = is.oneOf(
    is.object({
        type: 'exec',
        program: is.string,
    } as const),
    is.object({
        type: 'http',
        url: is.string,
        method: is.oneOf('GET', 'POST', 'PUT', 'DELETE', 'PATCH'),
    } as const),
);

const hostHookConfigChecker = is.object({
    action: hostHookActionChecker,
    delaySecs: is.number,
    timeoutSecs: is.number,
} as const);

export type HostHookConfig = Infer<typeof hostHookConfigChecker>;

const hostConfigChecker = is.object({
    enforceState: is.boolean,
    preStartup: is.optional(hostHookConfigChecker),
    postShutdown: is.optional(hostHookConfigChecker),
} as const);

export type HostConfig = Infer<typeof hostConfigChecker>;

const dbDataChecker = is.object({
    clientStats: is.recordOf(clientStatsChecker),
    hostStats: is.recordOf(hostStatChecker),
} as const);

export type DbData = Infer<typeof dbDataChecker>;

const dbDataStateChecker = is.oneOf(
    is.object({ status: 'disabled' } as const),
    is.object({
        status: 'available',
        payload: dbDataChecker,
    } as const),
    is.object({
        status: 'error',
        payload: is.object({ message: is.string } as const),
    } as const),
);

export type DbDataState = Infer<typeof dbDataStateChecker>;

/** The configuration values (mapped to what the frontend can see) that the coordinator accepts runtime changes of */
const dynamicConfigCheckerObj = {
    hosts: is.arrayOf(is.string),
    clients: is.arrayOf(is.string),
    hostConfigMap: is.recordOf(hostConfigChecker),
} as const;
const dynamicConfigChecker = is.object(dynamicConfigCheckerObj);

const operationFailureChecker = is.object({
    operation: is.oneOf('shutdown', 'startup'),
} as const);

const appStateChecker = is.object({
    statusMap: statusMapChecker,
    leaseMap: is.recordOf(is.arrayOf(leaseSourceChecker)),
    dbData: dbDataStateChecker,
    operationFailures: is.recordOf(operationFailureChecker),
    ...dynamicConfigCheckerObj,
} as const);

export type AppState = Infer<typeof appStateChecker>;

export type OperationFailure = Infer<typeof operationFailureChecker>;

const wsMessageChecker = is.oneOf(
    is.object({ type: 'HostStatus', payload: statusMapChecker } as const),
    is.object({
        type: 'ClientStats',
        payload: is.recordOf(clientStatsChecker),
    } as const),
    is.object({
        type: 'HostStats',
        payload: is.object({ host: is.string, stats: hostStatChecker }),
    } as const),
    is.object({
        type: 'ConfigChanged',
        payload: dynamicConfigChecker,
    } as const),
    is.object({ type: 'Initial', payload: appStateChecker } as const),
    is.object({
        type: 'LeaseUpdate',
        payload: is.object({
            host: is.string,
            leases: is.arrayOf(leaseSourceChecker),
        }),
    } as const),
    is.object({
        type: 'OperationFailed',
        payload: is.recordOf(operationFailureChecker),
    } as const),
);

export type WsMessage = Infer<typeof wsMessageChecker>;

const validateWsMessageAsync = (unknownMessage: unknown) =>
    validateDataAsync('ws-message', unknownMessage, wsMessageChecker);

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
    hostConfigMap: {},
});

export { state };

export const applyMessage = (unknownMessage: unknown) => {
    validateWsMessageAsync(unknownMessage);
    applyTypedMessage(unknownMessage as WsMessage);
};

export const applyTypedMessage = (message: WsMessage) => {
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
            setState('hostConfigMap', message.payload.hostConfigMap);
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
