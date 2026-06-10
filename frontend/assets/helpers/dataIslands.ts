/// <reference lib="dom" />

import { type Infer, is, validateData } from "./utils/assertData";

const authModeChecks = is.oneOf('token', 'oidc', 'disabled', 'external');

export type AuthMode = Infer<typeof authModeChecks>;

const serverDataChecks = is.object({
    configPath: is.string,
    authWarning: is.boolean,
    demoSubpath: is.optional(is.string),
    authMode: authModeChecks,
    broadcastPort: is.number,
    dbEnabled: is.boolean,
} as const);

type ServerData = Infer<typeof serverDataChecks>;

const loadServerData = (): ServerData => {
    if (typeof document === 'undefined') {
        // SSR context (generate-pages.tsx via vite-node) — no data needed for static rendering.
        return {} as ServerData;
    }
    const el = document.getElementById('server-data');
    if (!el?.textContent) throw new Error('Missing #server-data element');
    const parsed: unknown = JSON.parse(el.textContent);
    validateData('#server-data', parsed, serverDataChecks);
    return parsed;
};

/**
 * Data embedded by the server into the HTML for the client to read on startup.
 * This is used for configuration and should only contain static and non-sensitive data.
 *
 * `demoSubpath`:
 * Demo mode is encoded by presence of this field.
 * - `undefined` => normal mode
 * - `string` => demo mode (optional base subpath).
 */
export const serverData = loadServerData();


const buildDataChecks = is.object({
    stylesHash: is.string,
    stylesIntegrity: is.string,
    manifestHash: is.string,
    iconHashes: is.recordOf(is.string),
    svgHashes: is.recordOf(is.string),
    description: is.string,
    repository: is.string,
    version: is.string,
    appJsHash: is.string,
    appJsIntegrity: is.string,
} as const);

type BuildData = Infer<typeof buildDataChecks>;

const loadBuildData = () => {
    if (typeof document === 'undefined') {
        // SSR context (prerender.tsx via vite-node): return placeholder strings
        // for hash fields. Rust substitutes {{PRERENDERED_HTML}} first in the
        // template chain, so subsequent hash replacements resolve these too.
        return {
            svgHashes: { favicon: '{{FAVICON_SVG_HASH}}' },
            repository: '{{REPOSITORY_URL}}',
        } satisfies Partial<BuildData> as unknown as BuildData;
    }
    const el = document.getElementById('build-data');
    if (!el?.textContent) throw new Error('Missing #build-data element');
    const parsed: unknown = JSON.parse(el.textContent);
    validateData('#build-data', parsed, buildDataChecks);
    return parsed;
};

export const buildData = loadBuildData();
