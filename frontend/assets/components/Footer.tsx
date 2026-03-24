import type { Component } from 'solid-js';
import type { BuildData } from '../../generate-pages';

export const Footer: Component<{ data: BuildData }> = (props) => (
    <footer
        class="bg-white dark:bg-[#1e1e1e] shadow-md py-2 px-4 text-center text-[#616161] dark:text-[#a0a0a0] text-xs mt-auto"
        role="contentinfo"
    >
        <a href={props.data.repository} class="link">
            <span class="whitespace-nowrap">ShutHost Coordinator</span>
            <wbr />
            <span class="whitespace-nowrap"> v{props.data.version}</span>
        </a>
        <span aria-hidden="true"> | </span>
        <wbr />
        <a href="/about" rel="external" class="link font-medium whitespace-nowrap">About &amp; Licenses</a>
    </footer>
);
