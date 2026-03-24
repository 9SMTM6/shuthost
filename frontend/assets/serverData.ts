/// <reference lib="dom" />

export type ServerData = {
    configPath: string;
    showLogout: boolean;
    authWarning: boolean;
    isDemo: boolean;
    demoSubpath: string;
    authMode: 'token' | 'oidc' | 'disabled' | 'external';
    broadcastPort: number;
};

function loadServerData(): ServerData {
    if (typeof document === 'undefined') {
        // SSR context (generate-pages.tsx via vite-node) — no data needed for static rendering.
        return {} as ServerData;
    }
    const el = document.getElementById('server-data');
    if (!el?.textContent) throw new Error('Missing #server-data element');
    return JSON.parse(el.textContent) as ServerData;
}

export const serverData: ServerData = loadServerData();
