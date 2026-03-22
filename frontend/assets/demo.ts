import { applyMessage } from './stores/appStore';
import { serverData } from './serverData';

// Normalise subpath: '' or '/base' (no trailing slash)
const subpath = (() => {
    const raw = serverData.demoSubpath;
    if (!raw || raw === '/') return '';
    return (raw.startsWith('/') ? raw : '/' + raw).replace(/\/$/, '');
})();

export const demoBroadcastPort = 5757;

let leaseTimeout: ReturnType<typeof setTimeout> | null = null;
let statusTimeout: ReturnType<typeof setTimeout> | null = null;

export const initDemoMode = () => {
    // Adjust root-relative links for the GitHub Pages subpath
    document.querySelectorAll<HTMLAnchorElement>('a[href^="/"]').forEach(a => {
        const href = a.getAttribute('href');
        if (!href || href.startsWith('//')) return;
        a.setAttribute('href', subpath + href);
    });

    console.info('Demo mode enabled: UI is using simulated data.');

    // Simulate the Initial push from the backend
    setTimeout(() => {
        applyMessage({
            type: 'Initial',
            payload: {
                hosts: ['archive', 'tarbean'],
                clients: [],
                status: { tarbean: 'offline', archive: 'offline' },
                leases: { archive: [] },
                client_stats: {},
                broadcast_port: demoBroadcastPort,
            },
        });
    }, 500);
};

export const demoSubpath = subpath;

export const demoUpdateLease = async (host: string, action: 'take' | 'release') => {
    if (action === 'take') {
        if (leaseTimeout) clearTimeout(leaseTimeout);
        leaseTimeout = setTimeout(() => {
            applyMessage({ type: 'LeaseUpdate', payload: { host, leases: [{ type: 'WebInterface' }] } });
        }, 500);
        if (statusTimeout) clearTimeout(statusTimeout);
        statusTimeout = setTimeout(() => {
            applyMessage({ type: 'HostStatus', payload: { tarbean: 'offline', archive: 'online' } });
        }, 1200);
    } else {
        if (leaseTimeout) clearTimeout(leaseTimeout);
        leaseTimeout = setTimeout(() => {
            applyMessage({ type: 'LeaseUpdate', payload: { host, leases: [] } });
        }, 500);
        if (statusTimeout) clearTimeout(statusTimeout);
        statusTimeout = setTimeout(() => {
            applyMessage({ type: 'HostStatus', payload: { tarbean: 'offline', archive: 'offline' } });
        }, 1200);
    }
};
