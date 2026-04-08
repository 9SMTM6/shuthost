import { A, useLocation, useNavigate } from '@solidjs/router';
import { LogOut } from 'lucide-solid';
import type { JSX, ParentProps } from 'solid-js';
import { createSignal, Show } from 'solid-js';
import { isLoggedIn } from '../helpers/authState';
import { buildData } from '../helpers/buildData';
import type { AnyComponent } from '../helpers/component';
import { demoSubpath, isDemoMode } from '../helpers/demo';
import { type ServerData, serverData } from '../helpers/serverData';

const TAB_LABELS = {
    architecture: 'Docs',
    hosts: 'Hosts',
    clients: 'Clients',
} as const;

type TabId = keyof typeof TAB_LABELS;

const VALID_TABS = Object.keys(TAB_LABELS) as Array<TabId>;

const TAB_ROUTES = {
    architecture: '/docs',
    hosts: '/hosts',
    clients: '/clients',
} as const satisfies Record<TabId, string>;

/**
 * Shared header shell: branded bar with logo. Pass children to add content
 * after the logo (e.g. nav tabs, logout). leftExtra renders before the logo
 * (e.g. hamburger). topBanner renders inside <header> above the main bar.
 */
const HeaderShell = ((
    props: ParentProps<{ topBanner?: JSX.Element; leftExtra?: JSX.Element }>,
) => {
    const logoHref = () => (isLoggedIn() === false ? '/login' : '/');
    return (
        <header class="bg-white dark:bg-[#1e1e1e] shadow-md">
            {props.topBanner}
            <div class="max-w-full mx-auto px-4 sm:px-6 lg:px-8">
                <div class="flex items-center justify-between h-(--header-height)">
                    {props.leftExtra}
                    <A href={logoHref()} class="flex items-center gap-4">
                        <img
                            src={`${demoSubpath}/favicon.${buildData.svg_hashes['favicon']}.svg`}
                            alt="ShutHost Logo"
                            class="h-6 sm:h-8 w-auto"
                        />
                        <h1 class="text-xl sm:text-2xl font-semibold text-black dark:text-[#cccccc]">
                            ShutHost
                        </h1>
                    </A>
                    {props.children}
                </div>
            </div>
        </header>
    );
}) satisfies AnyComponent;

const SHOW_LOGOUT_FOR = {
    disabled: false,
    external: false,
    token: true,
    oidc: true,
} satisfies Record<ServerData['authMode'], boolean>;

/** Header for the About page: logo + conditional logout, no tab navigation. */
export const SimpleHeader = (() => (
    <HeaderShell>
        <Show
            when={isLoggedIn() === true && SHOW_LOGOUT_FOR[serverData.authMode]}
        >
            <form method="post" action="/logout">
                <button
                    type="submit"
                    class="text-xs sm:text-sm px-3 py-1 rounded border border-transparent btn btn-red"
                    aria-label="Logout"
                    title="Logout"
                >
                    <LogOut size={16} aria-hidden="true" />
                </button>
            </form>
        </Show>
    </HeaderShell>
)) satisfies AnyComponent;

/** Full header with tab navigation, logout button, and demo banner. */
export const Header = (() => {
    const location = useLocation();
    const navigate = useNavigate();

    const activeTab = (): TabId => {
        const path = location.pathname;
        if (path === '/clients' || path.startsWith('/clients/'))
            return 'clients';
        if (path === '/docs') return 'architecture';
        // /hosts, /hosts/:hostname, and any other path highlight the Hosts tab
        return 'hosts';
    };
    const [mobileMenuOpen, setMobileMenuOpen] = createSignal(false);

    const activateTab = (tabId: TabId) => {
        navigate(TAB_ROUTES[tabId], { scroll: false });
        setMobileMenuOpen(false);
    };

    const TabButton = ((tabProps: { tabId: TabId }) => (
        <button
            type="button"
            class="tab"
            classList={{ active: activeTab() === tabProps.tabId }}
            data-tab-content={tabProps.tabId}
            role="tab"
            aria-selected={activeTab() === tabProps.tabId}
            id={`tab-${tabProps.tabId}`}
            onClick={() => activateTab(tabProps.tabId)}
        >
            {TAB_LABELS[tabProps.tabId]}
        </button>
    )) satisfies AnyComponent;

    return (
        <>
            {/* Skip to main content link for accessibility */}
            <a
                href="#main-content"
                class="sr-only focus:not-sr-only absolute left-2 top-2 bg-white dark:bg-[#1e1e1e] text-[#005fb8] dark:text-[#4fc3f7] px-3 py-2 rounded z-50 shadow transition-all"
            >
                Skip to main content
            </a>

            <HeaderShell
                topBanner={
                    <Show when={isDemoMode}>
                        <div
                            id="demo-mode-disclaimer"
                            data-subpath={demoSubpath}
                            class="w-full bg-[#fff8e1] dark:bg-[rgba(204,167,0,0.15)] text-[#bf8803] dark:text-[#cca700] border border-[#ffd54f] dark:border-[#8a7300] px-4 py-2 text-center font-semibold"
                        >
                            Demo Mode: Static UI with simulated interactions
                            only
                        </div>
                    </Show>
                }
                leftExtra={
                    <button
                        type="button"
                        class="hamburger md:hidden"
                        classList={{ open: mobileMenuOpen() }}
                        aria-label="Toggle menu"
                        aria-expanded={mobileMenuOpen()}
                        onClick={() => setMobileMenuOpen((o) => !o)}
                    >
                        <span class="hamburger-line" />
                        <span class="hamburger-line" />
                        <span class="hamburger-line" />
                    </button>
                }
            >
                <div class="flex items-center gap-2">
                    <div
                        class="nav-tabs"
                        classList={{ open: mobileMenuOpen() }}
                        role="tablist"
                        aria-label="Main tabs"
                    >
                        {VALID_TABS.map((tabId) => (
                            <TabButton tabId={tabId} />
                        ))}
                    </div>
                    <Show when={SHOW_LOGOUT_FOR[serverData.authMode]}>
                        <form method="post" action="/logout">
                            <button
                                type="submit"
                                class="text-xs sm:text-sm px-3 py-1 rounded border border-transparent btn btn-red"
                                aria-label="Logout"
                                title="Logout"
                            >
                                <LogOut size={16} aria-hidden="true" />
                            </button>
                        </form>
                    </Show>
                </div>
            </HeaderShell>

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
}) satisfies AnyComponent;
