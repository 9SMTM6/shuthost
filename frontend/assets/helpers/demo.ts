import { applyMessage } from './appStore';
import { buildData } from './buildData';
import { serverData } from './serverData';

export const isDemoMode = serverData.demoSubpath != null;

const DEMO_SUBPATH_PATTERN = /^\/(?:[A-Za-z0-9_-]+(?:\/[A-Za-z0-9_-]+)*)$/;

const sanitizeDemoSubpath = (raw: string): string => {
    if (!raw || raw === '/') return '';

    const candidate = raw.startsWith('/') ? raw : `/${raw}`;
    if (!DEMO_SUBPATH_PATTERN.test(candidate)) {
        console.error(`Rejected invalid demoSubpath from serverData: ${raw}`);
        return '';
    }

    return candidate;
};

/** Normalised demo subpath: `''` or `'/base'` (no trailing slash). */
export const demoSubpath = sanitizeDemoSubpath(serverData.demoSubpath ?? '');

let leaseTimeout: ReturnType<typeof setTimeout> | null = null;
let statusTimeout: ReturnType<typeof setTimeout> | null = null;

let _demoInitialized = false;

export const initDemoMode = () => {
    if (_demoInitialized) return;
    _demoInitialized = true;

    console.info('Demo mode enabled: UI is using simulated data.');

    // Simulate the Initial push from the backend
    setTimeout(() => {
        applyMessage({
            type: 'Initial',
            payload: {
                hosts: ['archive', 'tarbean', 'junpui'],
                clients: [],
                statusMap: { tarbean: 'offline', archive: 'offline', junpui: 'offline' },
                leaseMap: { archive: [] },
                dbData: {
                    status: 'available',
                    payload: {
                            clientStats: {},
                            hostStats: {
                                archive: {
                                    agentVersion: '1.6.0',
                                    lastOnline: new Date(Date.now() - 3_600_000).toISOString(),
                                    isOnline: false,
                                    initSystem: 'systemd',
                                    operatingSystem: 'linux',
                                    scriptPath: undefined,
                                },
                                tarbean: {
                                    agentVersion: buildData.version,
                                    lastOnline: new Date(Date.now() - 7_200_000).toISOString(),
                                    initSystem: 'self-extracting-shell',
                                    operatingSystem: 'linux',
                                    scriptPath: '/home/user/shuthost_host_agent_self_extracting',
                                    isOnline: false,
                                },
                                junpui: {
                                    agentVersion: '1.6.0',
                                    initSystem: 'self-extracting-pwsh',
                                    operatingSystem: 'windows',
                                    scriptPath: 'C:\\Users\\user\\AppData\\Roaming\\shuthost\\shuthost_host_agent_self_extracting.ps1',
                                    lastOnline: new Date(Date.now() - 1_800_000).toISOString(),
                                    isOnline: false,
                                },
                            },
                        },
                },
            },
        });
    }, 500);
};

// ── Demo push subscription state ───────────────────────────────────────────
// In demo mode there is no real backend, so we maintain an in-memory set of
// subscribed hostnames and expose helpers that the push-subscription module
// can delegate to.

const demoPushSubscriptions = new Set<string>();

export const demoCheckHostUnscheduledSubscription = (
    hostname: string,
): boolean => demoPushSubscriptions.has(hostname);

export const demoSubscribeToHostUnscheduled = (hostname: string): void => {
    demoPushSubscriptions.add(hostname);
};

export const demoUnsubscribeFromHostUnscheduled = (hostname: string): void => {
    demoPushSubscriptions.delete(hostname);
};

export const demoUpdateLease = async (
    host: string,
    action: 'take' | 'release',
) => {
    if (action === 'take') {
        if (leaseTimeout) clearTimeout(leaseTimeout);
        leaseTimeout = setTimeout(() => {
            applyMessage({
                type: 'LeaseUpdate',
                payload: { host, leases: [{ type: 'WebInterface' }] },
            });
        }, 500);
        if (statusTimeout) clearTimeout(statusTimeout);
        statusTimeout = setTimeout(() => {
            applyMessage({
                type: 'HostStatus',
                payload: { tarbean: 'offline', archive: 'online' },
            });
        }, 1200);
    } else {
        if (leaseTimeout) clearTimeout(leaseTimeout);
        leaseTimeout = setTimeout(() => {
            applyMessage({
                type: 'LeaseUpdate',
                payload: { host, leases: [] },
            });
        }, 500);
        if (statusTimeout) clearTimeout(statusTimeout);
        statusTimeout = setTimeout(() => {
            applyMessage({
                type: 'HostStatus',
                payload: { tarbean: 'offline', archive: 'offline' },
            });
        }, 1200);
    }
};
