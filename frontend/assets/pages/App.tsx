import type { Component } from 'solid-js';
import { Show, onMount } from 'solid-js';
import { serverData } from '../serverData';
import { connectWebSocket } from '../ws';
import { initDemoMode } from '../demo';
import { HostsTab } from './HostsTab';
import { ClientsTab } from './ClientsTab';
import { AuthWarningPanel } from './AuthWarningPanel';
import { ArchitectureTab } from './ArchitectureTab';

/** Root component rendered inside RootLayout's <main id="main-content">. */
export const App = (() => {
    onMount(() => {
        if (serverData.isDemo) {
            initDemoMode();
        } else {
            connectWebSocket();
        }
    });

    return (
        <>
            {/* Auth security warning */}
            <Show when={serverData.authWarning}>
                <AuthWarningPanel />
            </Show>

            <ArchitectureTab />

            <HostsTab configPath={serverData.configPath} />
            <ClientsTab configPath={serverData.configPath} />
        </>
    );
}) satisfies Component<any>;
