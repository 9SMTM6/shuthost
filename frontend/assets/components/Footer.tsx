import { A } from '@solidjs/router';
import { createResource, Show } from 'solid-js';
import { apiFetch } from '../helpers/apiFetch';
import { buildData } from '../helpers/buildData';
import { demoSubpath } from '../helpers/demo';
import type { AnyComponent } from '../helpers/solidUtils';
import { safeExternalUrl } from '../helpers/utils';
import { authStatus } from '../helpers/authState';

type LatestRelease = { tag_name: string; url: string };

export const Footer = (() => {
    // re-runs the fetch when authStatus changes
    const [latest] = createResource(authStatus, async () => {
        try {
            const res = await apiFetch(`${demoSubpath}/api/update`, {
                checkAndRedirectUnauthorized: false,
            });
            // with !checkAndRedirectUnauthorized apiFetch ignores 401 errors
            if (res.status === 401) return null;
            return (await res.json()) as LatestRelease;
        } catch {
            return null;
        }
    });

    return (
        <footer
            class="bg-white dark:bg-[#1e1e1e] shadow-md py-2 px-4 text-center text-[#616161] dark:text-[#a0a0a0] text-xs mt-auto"
            role="contentinfo"
        >
            <a
                href={safeExternalUrl(buildData.repository)}
                rel="external"
                class="link"
            >
                <span class="whitespace-nowrap">ShutHost Coordinator</span>
                <wbr />
                <span class="whitespace-nowrap"> v{buildData.version}</span>
            </a>
            <Show when={latest()}>
                {(release) => (
                    <>
                        <span aria-hidden="true"> · </span>
                        <a
                            href={safeExternalUrl(release().url)}
                            rel="external"
                            class="link"
                        >
                            ↑ {release().tag_name}
                        </a>
                    </>
                )}
            </Show>
            <span aria-hidden="true"> | </span>
            <wbr />
            <A href="/about" class="link font-medium whitespace-nowrap">
                About &amp; Licenses
            </A>
        </footer>
    );
}) satisfies AnyComponent;
