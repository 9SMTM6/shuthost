import { applyTypedMessage, state, type WsMessage } from './appStore';
import { buildData } from './buildData';
import { serverData } from './serverData';

export const isDemoMode = serverData.demoSubpath != null;

const DEMO_SUBPATH_PATTERN = /^\/(?:[A-Za-z0-9_-]+(?:\/[A-Za-z0-9_-]+)*)$/;

const sanitizeDemoSubpath = (raw: string) => {
    if (!raw || raw === '/') return '';

    const candidate = raw.replace(/^\/+/, '/').replace(/\/+$/, ''); // trim leading/trailing slashes, ensure leading slash
    if (!DEMO_SUBPATH_PATTERN.test(candidate)) {
        console.error(`Rejected invalid demoSubpath from serverData: ${raw}`);
        return '';
    }

    return candidate;
};

/** Normalised demo subpath: `''` or `'/base'` (no trailing slash). */
export const demoSubpath = sanitizeDemoSubpath(serverData.demoSubpath ?? '');

const leaseTimeouts = new Map<string, ReturnType<typeof setTimeout>>();
const statusTimeouts = new Map<string, ReturnType<typeof setTimeout>>();

let _demoInitialized = false;

export const initDemoMode = () => {
    if (_demoInitialized) return;
    _demoInitialized = true;

    console.info('Demo mode enabled: UI is using simulated data.');

    // Simulate the Initial push from the backend
    setTimeout(() => {
        applyTypedMessage({
            type: 'Initial',
            payload: {
                hosts: ['archive', 'tarbean', 'junpui'],
                clients: [],
                statusMap: {
                    tarbean: 'offline',
                    archive: 'offline',
                    junpui: 'offline',
                },
                leaseMap: { archive: [] },
                operationFailures: {},
                dbData: {
                    status: 'available',
                    payload: {
                        clientStats: {},
                        hostStats: {
                            archive: {
                                agentVersion: '1.6.0',
                                lastOnline: new Date(
                                    Date.now() - 3_600_000,
                                ).toISOString(),
                                isOnline: false,
                                initSystem: 'systemd',
                                operatingSystem: 'linux',
                            },
                            tarbean: {
                                agentVersion: buildData.version,
                                lastOnline: new Date(
                                    Date.now() - 7_200_000,
                                ).toISOString(),
                                initSystem: 'self-extracting-shell',
                                operatingSystem: 'linux',
                                scriptPath:
                                    '/home/user/shuthost_host_agent_self_extracting',
                                isOnline: false,
                            },
                            junpui: {
                                agentVersion: '1.6.0',
                                initSystem: 'self-extracting-pwsh',
                                operatingSystem: 'windows',
                                scriptPath:
                                    'C:\\Users\\user\\AppData\\Roaming\\shuthost\\shuthost_host_agent_self_extracting.ps1',
                                lastOnline: new Date(
                                    Date.now() - 1_800_000,
                                ).toISOString(),
                                isOnline: false,
                            },
                        },
                    },
                },
                hostConfigMap: {
                    archive: {
                        enforceState: true,
                        preStartup: {
                            action: {
                                type: 'http',
                                url: 'https://example.com/pre-startup',
                                method: 'POST',
                            },
                            delaySecs: 0,
                            timeoutSecs: 10,
                        },
                        postShutdown: {
                            action: {
                                type: 'exec',
                                program: '/home/user/disable-plug.sh',
                            },
                            delaySecs: 2,
                            timeoutSecs: 15,
                        },
                    },
                    tarbean: {
                        enforceState: false,
                    },
                    junpui: {
                        enforceState: false,
                        postShutdown: {
                            action: {
                                type: 'exec',
                                program: '/home/user/disable-plug.sh',
                            },
                            delaySecs: 1,
                            timeoutSecs: 15,
                        },
                    },
                },
            },
        } satisfies WsMessage);
    }, 500);
};

// ── Demo push subscription state ───────────────────────────────────────────

/** In demo mode there is no real backend, so we maintain an in-memory
 * fake backend with relevant state.*/
const demoBackendState = {
    pushSubscriptions: new Set<string>(),
    operationFailedSubscriptions: new Set<string>(),
    onlineForSubs: new Map<string, number>(),
    oneshotOnlineForSubs: new Map<string, number>(),
};

const demoRequests = {
    checkHostUnscheduledSubscription: (hostname: string) =>
        demoBackendState.pushSubscriptions.has(hostname),
    subscribeToHostUnscheduled: (hostname: string): void => {
        demoBackendState.pushSubscriptions.add(hostname);
    },
    unsubscribeFromHostUnscheduled: (hostname: string): void => {
        demoBackendState.pushSubscriptions.delete(hostname);
    },
    resetLeases: (clientId: string): void => {
        // Demo: clear leases out of the store directly
        const newLeaseMap = { ...state.leaseMap };
        for (const host of Object.keys(newLeaseMap)) {
            newLeaseMap[host] = (newLeaseMap[host] ?? []).filter(
                (l) => l.type !== 'Client' || l.value !== clientId,
            );
        }
        applyTypedMessage({
            type: 'ConfigChanged',
            payload: {
                hosts: state.hosts,
                clients: state.clients,
                hostConfigMap: state.hostConfigMap,
            },
        });
        // Force a LeaseUpdate for each host to clear the demo state
        for (const host of Object.keys(newLeaseMap)) {
            applyTypedMessage({
                type: 'LeaseUpdate',
                payload: { host, leases: newLeaseMap[host] ?? [] },
            });
        }
    },
    checkHostOperationFailedSubscription: (hostname: string) =>
        demoBackendState.operationFailedSubscriptions.has(hostname),
    subscribeToHostOperationFailed: (hostname: string): void => {
        demoBackendState.operationFailedSubscriptions.add(hostname);
    },
    unsubscribeFromHostOperationFailed: (hostname: string): void => {
        demoBackendState.operationFailedSubscriptions.delete(hostname);
    },
    checkHostOnlineForSubscription: (hostname: string) =>
        demoBackendState.onlineForSubs.get(hostname) ?? null,
    subscribeToHostOnlineFor: (
        hostname: string,
        durationSecs: number,
    ): void => {
        demoBackendState.onlineForSubs.set(hostname, durationSecs);
    },
    unsubscribeFromHostOnlineFor: (hostname: string): void => {
        demoBackendState.onlineForSubs.delete(hostname);
    },
    subscribeToHostOnlineForOneshot: (
        hostname: string,
        durationSecs: number,
    ): void => {
        demoBackendState.oneshotOnlineForSubs.set(hostname, durationSecs);
    },
    updateLease: async (host: string, action: 'take' | 'release') => {
        const clearHostTimeouts = () => {
            const lt = leaseTimeouts.get(host);
            if (lt != null) clearTimeout(lt);
            const st = statusTimeouts.get(host);
            if (st != null) clearTimeout(st);
        };

        if (action === 'take') {
            clearHostTimeouts();
            leaseTimeouts.set(
                host,
                setTimeout(() => {
                    applyTypedMessage({
                        type: 'LeaseUpdate',
                        payload: { host, leases: [{ type: 'WebInterface' }] },
                    });
                }, 300),
            );
            statusTimeouts.set(
                host,
                setTimeout(() => {
                    applyTypedMessage({
                        type: 'HostStatus',
                        payload: { [host]: 'waking' },
                    });
                    statusTimeouts.set(
                        host,
                        setTimeout(() => {
                            applyTypedMessage({
                                type: 'HostStatus',
                                payload: { [host]: 'online' },
                            });
                        }, 1500),
                    );
                }, 300),
            );
        } else {
            clearHostTimeouts();
            leaseTimeouts.set(
                host,
                setTimeout(() => {
                    applyTypedMessage({
                        type: 'LeaseUpdate',
                        payload: { host, leases: [] },
                    });
                }, 300),
            );
            statusTimeouts.set(
                host,
                setTimeout(() => {
                    applyTypedMessage({
                        type: 'HostStatus',
                        payload: { [host]: 'shutting_down' },
                    });
                    statusTimeouts.set(
                        host,
                        setTimeout(() => {
                            applyTypedMessage({
                                type: 'HostStatus',
                                payload: { [host]: 'offline' },
                            });
                        }, 1500),
                    );
                }, 300),
            );
        }
    },
} as const;

export { demoRequests as demo };
