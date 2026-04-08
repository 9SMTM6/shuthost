/// <reference lib="dom" />

/**
 * Data embedded by the server into the HTML for the client to read on startup.
 * This is used for configuration and should only contain static and non-sensitive data.
 */
export type ServerData = {
    configPath: string;
    authWarning: boolean;
    /** Demo mode is encoded by presence of this field.
     *
     * * `undefined` => normal mode
     * * string => demo mode (optional base subpath).
     */
    demoSubpath?: string;
    authMode: 'token' | 'oidc' | 'disabled' | 'external';
    broadcastPort: number;
    dbEnabled: boolean;
};

const loadServerData = () => {
    if (typeof document === 'undefined') {
        // SSR context (generate-pages.tsx via vite-node) — no data needed for static rendering.
        return {} as ServerData;
    }
    const el = document.getElementById('server-data');
    if (!el?.textContent) throw new Error('Missing #server-data element');
    return JSON.parse(el.textContent) as ServerData;
};

export const serverData = loadServerData();
