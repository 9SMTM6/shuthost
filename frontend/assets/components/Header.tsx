import type { Component } from 'solid-js';
import { Show, createSignal, createEffect } from 'solid-js';
import { useLocation, useNavigate } from '@solidjs/router';
import { serverData } from '../serverData';
import { demoSubpath } from '../demo';
import { SimpleHeader } from './SimpleHeader';

export type HeaderVariant = 'full' | 'simple';

const VALID_TABS = ['architecture', 'hosts', 'clients'] as const;
type TabId = (typeof VALID_TABS)[number];

function normalizeTab(hash: string): TabId {
    const tab = hash.replace('#', '') as TabId;
    return VALID_TABS.includes(tab) ? tab : 'hosts';
}

/** Full header with tab navigation, logout button, and demo banner. */
const FullHeader: Component = () => {
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

    return (
        <>
            {/* Skip to main content link for accessibility */}
            <a
                href="#main-content"
                class="sr-only focus:not-sr-only absolute left-2 top-2 bg-white dark:bg-[#1e1e1e] text-[#005fb8] dark:text-[#4fc3f7] px-3 py-2 rounded z-50 shadow transition-all"
            >
                Skip to main content
            </a>

            {/* Demo disclaimer banner */}
            <Show when={serverData.isDemo}>
                <div
                    id="demo-mode-disclaimer"
                    data-subpath={demoSubpath}
                    class="w-full bg-[#fff8e1] dark:bg-[rgba(204,167,0,0.15)] text-[#bf8803] dark:text-[#cca700] border border-[#ffd54f] dark:border-[#8a7300] px-4 py-2 text-center font-semibold"
                >
                    Demo Mode: Static UI with simulated interactions only
                </div>
            </Show>

            <header class="bg-white dark:bg-[#1e1e1e] shadow-md" role="banner">
                <div class="max-w-full mx-auto px-4 sm:px-6 lg:px-8">
                    <div class="flex items-center justify-between h-(--header-height)">
                        <div class="flex items-center gap-4">
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
                            <a href="/" class="flex items-center gap-4">
                                <img src="/favicon.svg" alt="ShutHost Logo" class="h-6 sm:h-8 w-auto" />
                                <h1 class="text-xl sm:text-2xl font-semibold text-black dark:text-[#cccccc]">
                                    ShutHost
                                </h1>
                            </a>
                        </div>
                        <div class="flex items-center gap-2">
                            <nav class="nav-tabs" role="tablist" aria-label="Main tabs">
                                <button
                                    type="button"
                                    class="tab"
                                    data-tab-content="architecture"
                                    role="tab"
                                    aria-selected={activeTab() === 'architecture'}
                                    aria-controls="architecture-tab"
                                    id="tab-architecture"
                                    onClick={() => activateTab('architecture')}
                                >
                                    Docs
                                </button>
                                <button
                                    type="button"
                                    class="tab"
                                    data-tab-content="hosts"
                                    role="tab"
                                    aria-selected={activeTab() === 'hosts'}
                                    aria-controls="hosts-tab"
                                    id="tab-hosts"
                                    onClick={() => activateTab('hosts')}
                                >
                                    Hosts
                                </button>
                                <button
                                    type="button"
                                    class="tab"
                                    data-tab-content="clients"
                                    role="tab"
                                    aria-selected={activeTab() === 'clients'}
                                    aria-controls="clients-tab"
                                    id="tab-clients"
                                    onClick={() => activateTab('clients')}
                                >
                                    Clients
                                </button>
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
                    </div>
                </div>
                {/* Mobile menu */}
                <Show when={mobileMenuOpen()}>
                    <div class="md:hidden border-t border-[#e5e5e5] dark:border-[#3e3e42] px-4 py-2 flex flex-col gap-1">
                        <button
                            type="button"
                            class="tab text-left w-full"
                            onClick={() => activateTab('architecture')}
                        >
                            Docs
                        </button>
                        <button
                            type="button"
                            class="tab text-left w-full"
                            onClick={() => activateTab('hosts')}
                        >
                            Hosts
                        </button>
                        <button
                            type="button"
                            class="tab text-left w-full"
                            onClick={() => activateTab('clients')}
                        >
                            Clients
                        </button>
                    </div>
                </Show>
            </header>
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

export const Header: Component<{ variant?: HeaderVariant }> = (props) => {
    if (props.variant === 'simple') {
        return <SimpleHeader />;
    }
    return <FullHeader />;
};

export { SimpleHeader };
