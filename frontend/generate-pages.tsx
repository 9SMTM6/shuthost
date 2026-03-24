/**
 * vite-node script that generates static HTML pages at build time.
 *
 * Run via: npm run generate-pages
 *
 * Produces:
 *   assets/generated/index.html  — SPA shell for the coordinator UI
 *   assets/generated/about.html  — statically rendered about/licenses page
 */

import { readFileSync, writeFileSync } from 'fs';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';
import { renderToString } from 'solid-js/web';

import { HtmlHead } from './assets/components/HtmlHead';
import { Footer } from './assets/components/Footer';
import { SimpleHeader } from './assets/components/Header';
import { AboutPage, type AboutPageProps } from './assets/pages/AboutPage';

export interface BuildData {
    styles_hash: string;
    styles_integrity: string;
    manifest_hash: string;
    icon_hashes: Record<string, string>;
    svg_hashes: Record<string, string>;
    description: string;
    repository: string;
    version: string;
}

export function loadBuildData(): BuildData {
    const path = resolve(frontend_dir, 'assets/generated/build-data.json');
    return JSON.parse(readFileSync(path, 'utf-8')) as BuildData;
}

const frontend_dir = dirname(fileURLToPath(import.meta.url));

function asset(path: string): string {
    return resolve(frontend_dir, path);
}

// ──────────────────────────────────────────────────────────────────────────────
// Shared resources
// ──────────────────────────────────────────────────────────────────────────────

const buildData = loadBuildData();
const appJs = readFileSync(asset('assets/generated/app.js'), 'utf-8');

// ──────────────────────────────────────────────────────────────────────────────
// Page assembly helpers
// ──────────────────────────────────────────────────────────────────────────────

function escapeHtml(str: string): string {
    return str
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;');
}

interface PageOptions {
    title: string;
    head: string;
    bodyClass?: string;
    bodyContent: string;
    footer?: string;
}

function buildPage(opts: PageOptions): string {
    const bodyClass = opts.bodyClass ? ` class="${escapeHtml(opts.bodyClass)}"` : '';
    const footerHtml = opts.footer ? `\n${opts.footer}` : '';
    return `<!DOCTYPE html>
<html lang="en">

${opts.head}

<body${bodyClass}>
${opts.bodyContent}${footerHtml}
</body>

</html>
`;
}

// ──────────────────────────────────────────────────────────────────────────────
// index.html — SPA shell
// ──────────────────────────────────────────────────────────────────────────────

const head = (title: string) => renderToString(() => <HtmlHead title={title} data={buildData} />);
const footer = renderToString(() => <Footer data={buildData} />);

// The literal string `{ server_data }` is preserved here for Rust's runtime .replace() in render_ui_html().
const indexBodyContent = `\
<noscript>
    <div id="noscript-warning" class="alert alert-error mb-4" role="alert">
        <strong class="alert-title">Error!</strong>
        <p>This application requires JavaScript to function properly. Please enable JavaScript in your browser settings
            and reload the page.</p>
    </div>
</noscript>
    <div id="app"></div>
    <script id="server-data" type="application/json">{ server_data }</script>
    <script type="module">
${appJs}
    </script>`;

const indexHtml = buildPage({
    title: 'ShutHost Coordinator',
    head: head('ShutHost Coordinator'),
    bodyContent: indexBodyContent,
    footer,
});

writeFileSync(asset('assets/generated/index.html'), indexHtml);
console.log('Generated: assets/generated/index.html');

// ──────────────────────────────────────────────────────────────────────────────
// about.html — statically rendered via SolidJS renderToString
// ──────────────────────────────────────────────────────────────────────────────

const aboutData: AboutPageProps = JSON.parse(
    readFileSync(asset('assets/generated/about-data.json'), 'utf-8'),
);

const aboutHeader = renderToString(() => SimpleHeader({}));
const aboutMain = renderToString(() => AboutPage(aboutData));

const aboutBodyContent = `\
${aboutHeader}
${aboutMain}`;

const aboutHtml = buildPage({
    title: 'Dependencies and Licenses • ShutHost',
    head: head('Dependencies and Licenses • ShutHost'),
    bodyClass: 'disable-nav',
    bodyContent: aboutBodyContent,
    footer,
});

writeFileSync(asset('assets/generated/about.html'), aboutHtml);
console.log('Generated: assets/generated/about.html');
