/// <reference lib="dom" />

import { assertData, is, type Infer } from './assertData';

const buildDataChecks = {
    stylesHash:       is.string,
    stylesIntegrity:  is.string,
    manifestHash:     is.string,
    iconHashes:       is.recordOf(is.string),
    svgHashes:        is.recordOf(is.string),
    description:       is.string,
    repository:        is.string,
    version:           is.string,
    appJsHash:        is.string,
    appJsIntegrity:   is.string,
} as const;

export type BuildData = Infer<typeof buildDataChecks>;

const loadBuildData = (): BuildData => {
    if (typeof document === 'undefined') {
        // SSR context (prerender.tsx via vite-node): return placeholder strings
        // for hash fields. Rust substitutes {{PRERENDERED_HTML}} first in the
        // template chain, so subsequent hash replacements resolve these too.
        return {
            svgHashes: { favicon: '{{FAVICON_SVG_HASH}}' },
        } as unknown as BuildData;
    }
    const el = document.getElementById('build-data');
    if (!el?.textContent) throw new Error('Missing #build-data element');
    const parsed: unknown = JSON.parse(el.textContent);
    assertData('#build-data', parsed, buildDataChecks);
    return parsed;
};

export const buildData: BuildData = loadBuildData();
