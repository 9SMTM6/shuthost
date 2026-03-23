/**
 * Shared HTML assembly utilities for generate-pages.tsx.
 * No JSX — pure TypeScript string building.
 */

import { readFileSync } from 'fs';
import { resolve, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));

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
    const path = resolve(__dirname, 'assets/generated/build-data.json');
    return JSON.parse(readFileSync(path, 'utf-8')) as BuildData;
}

/** Replace `{ asset_name }` placeholders with hashed SVG URLs (`./name.HASH.svg`). */
export function replaceSvgHashes(content: string, svgHashes: Record<string, string>): string {
    let result = content;
    for (const [asset, hash] of Object.entries(svgHashes)) {
        result = result.replaceAll(`{ ${asset} }`, `./${asset}.${hash}.svg`);
    }
    return result;
}

/** Replace `{ icon_N }` placeholders with hashed PNG icon URLs. */
export function replaceIconHashes(content: string, iconHashes: Record<string, string>): string {
    let result = content;
    for (const [size, hash] of Object.entries(iconHashes)) {
        result = result.replaceAll(`{ icon_${size} }`, `./icons/icon-${size}.${hash}.png`);
    }
    return result;
}

/** Build the full `<head>` block for a page. */
export function renderHtmlHead(title: string, data: BuildData): string {
    const { styles_hash, styles_integrity, manifest_hash, icon_hashes, svg_hashes, description } = data;
    const favicon = `./favicon.${svg_hashes['favicon']}.svg`;
    const manifest = `./manifest.${manifest_hash}.json`;
    const styles = `./styles.${styles_hash}.css`;

    return `<head>
    <meta charset="UTF-8">
    <title>${escapeHtml(title)}</title>
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <meta name="description" content="${escapeHtml(description)}">
    <meta name="theme-color" media="(prefers-color-scheme: light)" content="#0b6b3a">
    <meta name="theme-color" media="(prefers-color-scheme: dark)" content="#2ec164">
    <meta name="background-color" media="(prefers-color-scheme: light)" content="#ffffff">
    <meta name="background-color" media="(prefers-color-scheme: dark)" content="#0b0f12">
    <link rel="manifest" href="${manifest}">
    <link rel="icon" href="./icons/icon-32.${icon_hashes['32']}.png" sizes="32x32" type="image/png">
    <link rel="icon" href="./icons/icon-48.${icon_hashes['48']}.png" sizes="48x48" type="image/png">
    <link rel="icon" href="./icons/icon-64.${icon_hashes['64']}.png" sizes="64x64" type="image/png">
    <link rel="icon" href="./icons/icon-128.${icon_hashes['128']}.png" sizes="128x128" type="image/png">
    <link rel="apple-touch-icon" href="./icons/icon-180.${icon_hashes['180']}.png" sizes="180x180">
    <link rel="icon" href="${favicon}" type="image/svg+xml">
    <link rel="stylesheet" href="${styles}" integrity="${styles_integrity}">
</head>`;
}

/** Render the site footer. */
export function renderFooter(data: BuildData): string {
    const { repository, version } = data;
    return `<footer
    class="bg-white dark:bg-[#1e1e1e] shadow-md py-2 px-4 text-center text-[#616161] dark:text-[#a0a0a0] text-xs mt-auto"
    role="contentinfo">
    <a href="${escapeHtml(repository)}" class="link">
        <span class="whitespace-nowrap">ShutHost Coordinator</span><wbr><span class="whitespace-nowrap"> v${escapeHtml(version)}</span>
    </a>
    <span class="mx-2" aria-hidden="true">|</span><wbr>
    <a href="/about" rel="external" class="link font-medium whitespace-nowrap">About &amp; Licenses</a>
</footer>`;
}

export interface PageOptions {
    title: string;
    head: string;
    bodyClass?: string;
    bodyContent: string;
    footer?: string;
}

/** Assemble a full HTML document. */
export function buildPage(opts: PageOptions): string {
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

function escapeHtml(str: string): string {
    return str
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;');
}
