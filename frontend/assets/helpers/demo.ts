import { applyMessage } from './appStore';
import { serverData } from './serverData';

export const isDemoMode = serverData.demoSubpath != null;

/** Normalised demo subpath: `''` or `'/base'` (no trailing slash). */
export const demoSubpath = (() => {
    const raw = serverData.demoSubpath ?? '';
    if (!raw || raw === '/') return '';
    return (raw.startsWith('/') ? raw : `/${raw}`).replace(/\/$/, '');
})();

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
                hosts: ['archive', 'tarbean'],
                clients: [],
                statusMap: { tarbean: 'offline', archive: 'offline' },
                leaseMap: { archive: [] },
                dbData: { clientStats: {}, hostStats: {} },
            },
        });
    }, 500);
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
