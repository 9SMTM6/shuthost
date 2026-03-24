import type { Component, ParentComponent, JSX } from 'solid-js';
import { Show, createSignal, createEffect } from 'solid-js';
import { useLocation, useNavigate } from '@solidjs/router';
import { serverData } from '../serverData';
import { demoSubpath } from '../demo';

const TAB_LABELS = {
    architecture: 'Docs',
    hosts: 'Hosts',
    clients: 'Clients',
} as const;

type TabId = keyof typeof TAB_LABELS;

const VALID_TABS = Object.keys(TAB_LABELS) as Array<TabId>;

function normalizeTab(hash: string): TabId {
    const tab = hash.replace('#', '') as TabId;
    return VALID_TABS.includes(tab) ? tab : 'hosts';
}

/**
 * Shared header shell: branded bar with logo. Pass children to add content
 * after the logo (e.g. nav tabs, hamburger). Simple variant passes nothing.
 * topBanner renders inside <header> above the main bar (keeps it in grid-area: header).
 */
const HeaderShell: ParentComponent<{ topBanner?: JSX.Element }> = (props) => (
    <header class="bg-white dark:bg-[#1e1e1e] shadow-md" role="banner">
        {props.topBanner}
        <div class="max-w-full mx-auto px-4 sm:px-6 lg:px-8">
            <div class="flex items-center justify-between h-(--header-height)">
                <a href="/" class="flex items-center gap-4">
                    <img src="/favicon.svg" alt="ShutHost Logo" class="h-6 sm:h-8 w-auto" />
                    <h1 class="text-xl sm:text-2xl font-semibold text-black dark:text-[#cccccc]">
                        ShutHost
                    </h1>
                </a>
                {props.children}
            </div>
        </div>
    </header>
);

/** Minimal header for static/about pages. No router dependency — safe for SSR (generate-pages). */
export const SimpleHeader: Component = () => <HeaderShell />;

/** Full header with tab navigation, logout button, and demo banner. */
export const Header: Component = () => {
    const location = useLocation();
    const navigate = useNavigate();

    const activeTab = () => normalizeTab(location.hash);
    const [mobileMenuOpen, setMobileMenuOpen] = createSignal(false);

    const activateTab = (tabId: TabId) => {
        navigate(`${location.pathname}#${tabId}`, { replace: true, scroll: false });
        setMobileMenuOpen(false);
    };

    // Show/hide .tab-content elements (including #architecture-tab outside SolidJS tree)
    createEffect(() => {
        const tab = activeTab();
        document.querySelectorAll<HTMLElement>('.tab-content').forEach(el => {
            const isActive = el.id === `${tab}-tab`;
            el.classList.toggle('active', isActive);
            el.setAttribute('aria-hidden', String(!isActive));
        });
        document.querySelectorAll<HTMLElement>('.tab').forEach(btn => {
            const tabId = btn.dataset['tabContent'] as TabId | undefined;
            const isActive = tabId === tab;
            btn.classList.toggle('active', isActive);
            btn.setAttribute('aria-selected', String(isActive));
        });
    });

    const TabButton: Component<{ tabId: TabId; mobile?: boolean }> = (tabProps) => (
        <button
            type="button"
            class={tabProps.mobile ? 'tab text-left w-full' : 'tab'}
            data-tab-content={tabProps.mobile ? undefined : tabProps.tabId}
            role={tabProps.mobile ? undefined : 'tab'}
            aria-selected={tabProps.mobile ? undefined : activeTab() === tabProps.tabId}
            aria-controls={tabProps.mobile ? undefined : `${tabProps.tabId}-tab`}
            id={tabProps.mobile ? undefined : `tab-${tabProps.tabId}`}
            onClick={() => activateTab(tabProps.tabId)}
        >
            {TAB_LABELS[tabProps.tabId]}
        </button>
    );

    return (
        <>
            {/* Skip to main content link for accessibility */}
            <a
                href="#main-content"
                class="sr-only focus:not-sr-only absolute left-2 top-2 bg-white dark:bg-[#1e1e1e] text-[#005fb8] dark:text-[#4fc3f7] px-3 py-2 rounded z-50 shadow transition-all"
            >
                Skip to main content
            </a>

            <HeaderShell topBanner={
                <Show when={serverData.isDemo}>
                    <div
                        id="demo-mode-disclaimer"
                        data-subpath={demoSubpath}
                        class="w-full bg-[#fff8e1] dark:bg-[rgba(204,167,0,0.15)] text-[#bf8803] dark:text-[#cca700] border border-[#ffd54f] dark:border-[#8a7300] px-4 py-2 text-center font-semibold"
                    >
                        Demo Mode: Static UI with simulated interactions only
                    </div>
                </Show>
            }>
                <div class="flex items-center gap-2">
                    <button
                        type="button"
                        class="hamburger md:hidden"
                        aria-label="Toggle menu"
                        aria-expanded={mobileMenuOpen()}
                        onClick={() => setMobileMenuOpen(o => !o)}
                    >
                        <span class="hamburger-line" />
                        <span class="hamburger-line" />
                        <span class="hamburger-line" />
                    </button>
                    <nav class="nav-tabs" role="tablist" aria-label="Main tabs">
                        {VALID_TABS.map(tabId => <TabButton tabId={tabId} />)}
                    </nav>
                    <Show when={serverData.showLogout}>
                        <form method="post" action="/logout">
                            <button
                                type="submit"
                                class="text-xs sm:text-sm px-3 py-1 rounded border border-transparent btn btn-red"
                                aria-label="Logout"
                            >
                                Logout
                            </button>
                        </form>
                    </Show>
                </div>
            </HeaderShell>

            {/* Mobile menu */}
            <Show when={mobileMenuOpen()}>
                <div class="md:hidden border-t border-[#e5e5e5] dark:border-[#3e3e42] px-4 py-2 flex flex-col gap-1">
                    {VALID_TABS.map(tabId => <TabButton tabId={tabId} mobile />)}
                </div>
            </Show>

            {/* Mobile menu backdrop */}
            <Show when={mobileMenuOpen()}>
                <div
                    class="mobile-menu-backdrop"
                    onClick={() => setMobileMenuOpen(false)}
                    aria-hidden="true"
                />
            </Show>
        </>
    );
};
