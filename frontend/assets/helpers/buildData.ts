/// <reference lib="dom" />

export type BuildData = {
    styles_hash: string;
    styles_integrity: string;
    manifest_hash: string;
    icon_hashes: Record<number, string>;
    svg_hashes: Record<string, string>;
    description: string;
    repository: string;
    version: string;
};

const loadBuildData = () => {
    if (typeof document === 'undefined') {
        // SSR context (generate-pages.tsx via vite-node) — no data needed for static rendering.
        return {} as BuildData;
    }
    const el = document.getElementById('build-data');
    if (!el?.textContent) throw new Error('Missing #build-data element');
    return JSON.parse(el.textContent) as BuildData;
};

export const buildData = loadBuildData();
