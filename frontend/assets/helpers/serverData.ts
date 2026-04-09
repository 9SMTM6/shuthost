/// <reference lib="dom" />

import { assertData, is, type Infer } from './assertData';

/**
 * Data embedded by the server into the HTML for the client to read on startup.
 * This is used for configuration and should only contain static and non-sensitive data.
 */
const serverDataChecks = {
    configPath:    is.string,
    authWarning:   is.boolean,
    /** Demo mode is encoded by presence of this field.
     * - `undefined` => normal mode
     * - `string` => demo mode (optional base subpath).
     */
    demoSubpath:   is.optional(is.string),
    authMode:      is.oneOf('token', 'oidc', 'disabled', 'external'),
    broadcastPort: is.number,
    dbEnabled:     is.boolean,
} as const;

export type ServerData = Infer<typeof serverDataChecks>;

const loadServerData = () => {
    if (typeof document === 'undefined') {
        // SSR context (generate-pages.tsx via vite-node) — no data needed for static rendering.
        return {} as ServerData;
    }
    const el = document.getElementById('server-data');
    if (!el?.textContent) throw new Error('Missing #server-data element');
    const parsed: unknown = JSON.parse(el.textContent);
    assertData('#server-data', parsed, serverDataChecks);
    return parsed;
};

export const serverData = loadServerData();
