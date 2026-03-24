import type { Component } from 'solid-js';
import type { BuildData } from '../../generate-pages';

export const HtmlHead: Component<{ title: string; data: BuildData }> = (props) => {
    const d = props.data;
    return (
        <head>
            <meta charset="UTF-8" />
            <title>{props.title}</title>
            <meta name="viewport" content="width=device-width, initial-scale=1.0" />
            <meta name="description" content={d.description} />
            <meta name="theme-color" media="(prefers-color-scheme: light)" content="#0b6b3a" />
            <meta name="theme-color" media="(prefers-color-scheme: dark)" content="#2ec164" />
            <meta name="background-color" media="(prefers-color-scheme: light)" content="#ffffff" />
            <meta name="background-color" media="(prefers-color-scheme: dark)" content="#0b0f12" />
            <link rel="manifest" href={`./manifest.${d.manifest_hash}.json`} />
            <link rel="icon" href={`./icons/icon-32.${d.icon_hashes['32']}.png`} sizes="32x32" type="image/png" />
            <link rel="icon" href={`./icons/icon-48.${d.icon_hashes['48']}.png`} sizes="48x48" type="image/png" />
            <link rel="icon" href={`./icons/icon-64.${d.icon_hashes['64']}.png`} sizes="64x64" type="image/png" />
            <link rel="icon" href={`./icons/icon-128.${d.icon_hashes['128']}.png`} sizes="128x128" type="image/png" />
            <link rel="apple-touch-icon" href={`./icons/icon-180.${d.icon_hashes['180']}.png`} sizes="180x180" />
            <link rel="icon" href={`./favicon.${d.svg_hashes['favicon']}.svg`} type="image/svg+xml" />
            <link rel="stylesheet" href={`./styles.${d.styles_hash}.css`} integrity={d.styles_integrity} />
        </head>
    );
};
