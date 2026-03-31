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
    app_js_hash: string;
    app_js_integrity: string;
};

const loadBuildData = (): BuildData => {
    if (typeof document === 'undefined') {
        // SSR context (prerender.tsx via vite-node): return placeholder strings
        // for hash fields. Rust substitutes {{PRERENDERED_HTML}} first in the
        // template chain, so subsequent hash replacements resolve these too.
        return {
            svg_hashes: { favicon: '{{FAVICON_SVG_HASH}}' },
        } as unknown as BuildData;
    }
    const el = document.getElementById('build-data');
    if (!el?.textContent) throw new Error('Missing #build-data element');
    return JSON.parse(el.textContent) as BuildData;
};

export const buildData: BuildData = loadBuildData();
