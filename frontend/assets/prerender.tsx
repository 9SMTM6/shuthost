/**
 * SSR prerender entry point — executed via `vite-node --config vite.config.ssr.ts` at build time.
 *
 * Renders PrerenderedShell using the buildData singleton, which in SSR context
 * returns placeholder strings (e.g. {{FAVICON_SVG_HASH}}) for hash fields.
 * Rust then substitutes {{PRERENDERED_HTML}} first in the template chain so
 * all subsequent hash replacements resolve those placeholders too.
 */
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { renderToString } from 'solid-js/web';
import { MetaProvider } from '@solidjs/meta';
import { Router, Route } from '@solidjs/router';
import { PrerenderedShell } from './pages/PrerenderedShell';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

const html = renderToString(() => (
    <MetaProvider>
        <Router url="/">
            <Route path="*" component={PrerenderedShell} />
        </Router>
    </MetaProvider>
));

const outDir = path.join(__dirname, 'generated');
fs.mkdirSync(outDir, { recursive: true });
fs.writeFileSync(path.join(outDir, 'prerendered-app.html'), html, 'utf-8');
console.log('Prerendering complete');
