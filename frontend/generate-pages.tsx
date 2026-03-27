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
import { MetaProvider } from '@solidjs/meta';

import { SimpleHeader } from './assets/components/Header';
import { AboutPage, type AboutPageProps } from './assets/pages/AboutPage';

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

export const loadBuildData = () => {
    const path = resolve(frontend_dir, 'assets/generated/build-data.json');
    return JSON.parse(readFileSync(path, 'utf-8')) as BuildData;
};

const frontend_dir = dirname(fileURLToPath(import.meta.url));

const asset = (path: string) => resolve(frontend_dir, path);

// ──────────────────────────────────────────────────────────────────────────────
// Shared resources
// ──────────────────────────────────────────────────────────────────────────────

const buildData = loadBuildData();
const appJs = readFileSync(asset('assets/generated/app.js'), 'utf-8');

// ──────────────────────────────────────────────────────────────────────────────
// Page assembly helpers
// ──────────────────────────────────────────────────────────────────────────────

const escapeHtml = (str: string) => str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');

type PageOptions = {
    title?: string;
    bodyContent: string;
};

const buildPage = (opts: PageOptions) => {
    const titleTag = opts.title ? `<title>${escapeHtml(opts.title)}</title>` : '';
    return `<!DOCTYPE html>
<html lang="en">

<head>
    <meta charset="UTF-8" />
    ${titleTag}
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <meta name="description" content="${buildData.description}" />
    <meta name="theme-color" media="(prefers-color-scheme: light)" content="#0b6b3a" />
    <meta name="theme-color" media="(prefers-color-scheme: dark)" content="#2ec164" />
    <meta name="background-color" media="(prefers-color-scheme: light)" content="#ffffff" />
    <meta name="background-color" media="(prefers-color-scheme: dark)" content="#0b0f12" />
    <link rel="manifest" href="./manifest.${buildData.manifest_hash}.json" />
    <link rel="icon" href="./icons/icon-32.${buildData.icon_hashes['32']}.png" sizes="32x32" type="image/png" />
    <link rel="icon" href="./icons/icon-48.${buildData.icon_hashes['48']}.png" sizes="48x48" type="image/png" />
    <link rel="icon" href="./icons/icon-64.${buildData.icon_hashes['64']}.png" sizes="64x64" type="image/png" />
    <link rel="icon" href="./icons/icon-128.${buildData.icon_hashes['128']}.png" sizes="128x128" type="image/png" />
    <link rel="apple-touch-icon" href="./icons/icon-180.${buildData.icon_hashes['180']}.png" sizes="180x180" />
    <link rel="icon" href="./favicon.${buildData.svg_hashes['favicon']}.svg" type="image/svg+xml" />
    <link rel="stylesheet" href="./styles.${buildData.styles_hash}.css" integrity="${buildData.styles_integrity}" />
</head>

<body>
${opts.bodyContent}
<footer
    class="bg-white dark:bg-[#1e1e1e] shadow-md py-2 px-4 text-center text-[#616161] dark:text-[#a0a0a0] text-xs mt-auto"
    role="contentinfo"
>
    <a href="${buildData.repository}" rel="external" class="link">
        <span class="whitespace-nowrap">ShutHost Coordinator</span>
        <wbr />
        <span class="whitespace-nowrap"> v${buildData.version}</span>
    </a>
    <span aria-hidden="true"> | </span>
    <wbr />
    <a href="/about" rel="external" class="link font-medium whitespace-nowrap">About &amp; Licenses</a>
</footer>
</body>

</html>
`;
};

// ──────────────────────────────────────────────────────────────────────────────
// index.html — SPA shell
// ──────────────────────────────────────────────────────────────────────────────

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
    <script type="module">${appJs}</script>`;

const indexHtml = buildPage({
    bodyContent: indexBodyContent,
});

writeFileSync(asset('assets/generated/index.html'), indexHtml);
console.log('Generated: assets/generated/index.html');

// ──────────────────────────────────────────────────────────────────────────────
// about.html — statically rendered via SolidJS renderToString
// ──────────────────────────────────────────────────────────────────────────────

const aboutData: AboutPageProps = JSON.parse(
    readFileSync(asset('assets/generated/about-data.json'), 'utf-8'),
);

const aboutHeader = renderToString(() => <SimpleHeader />);
const aboutMain = renderToString(() => (
    <MetaProvider>
        <AboutPage {...aboutData} />
    </MetaProvider>
));

const aboutBodyContent = `\
${aboutHeader}
${aboutMain}`;

const aboutHtml = buildPage({
    title: 'About & Licenses - ShutHost Coordinator',
    bodyContent: aboutBodyContent,
});

writeFileSync(asset('assets/generated/about.html'), aboutHtml);
console.log('Generated: assets/generated/about.html');
