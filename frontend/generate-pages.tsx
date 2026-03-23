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

import {
    loadBuildData,
    buildPage,
    renderHtmlHead,
    renderFooter,
    replaceSvgHashes,
} from './build-common';
import { SimpleHeader } from './assets/components/SimpleHeader';
import { AboutPage, type AboutPageProps } from './assets/pages/AboutPage';

const __dirname = dirname(fileURLToPath(import.meta.url));

function asset(path: string): string {
    return resolve(__dirname, path);
}

// ──────────────────────────────────────────────────────────────────────────────
// Shared resources
// ──────────────────────────────────────────────────────────────────────────────

const buildData = loadBuildData();
const jsWarnings = readFileSync(asset('assets/partials/js_warnings.html'), 'utf-8');
const appJs = readFileSync(asset('assets/generated/app.js'), 'utf-8');

// ──────────────────────────────────────────────────────────────────────────────
// index.html — SPA shell
// ──────────────────────────────────────────────────────────────────────────────

// Process architecture tab: replace placeholders with hashed URLs
const platformSupport = readFileSync(asset('assets/partials/platform_support.md'), 'utf-8');
let architectureHtml = readFileSync(asset('assets/partials/architecture.html'), 'utf-8');
architectureHtml = architectureHtml.replace('{ platform_support }', platformSupport);
architectureHtml = replaceSvgHashes(architectureHtml, buildData.svg_hashes);

const footer = renderFooter(buildData);

// The literal string `{ server_data }` is preserved here for Rust's runtime .replace() in render_ui_html().
const indexBodyContent = `\
${jsWarnings}
    <div id="app"></div>
${architectureHtml}
    <script id="server-data" type="application/json">{ server_data }</script>
    <script type="module">
${appJs}
    </script>`;

const indexHtml = buildPage({
    title: 'ShutHost Coordinator',
    head: renderHtmlHead('ShutHost Coordinator', buildData),
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
    head: renderHtmlHead('Dependencies and Licenses • ShutHost', buildData),
    bodyClass: 'disable-nav',
    bodyContent: aboutBodyContent,
    footer,
});

writeFileSync(asset('assets/generated/about.html'), aboutHtml);
console.log('Generated: assets/generated/about.html');
