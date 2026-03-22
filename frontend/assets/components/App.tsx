import type { Component } from 'solid-js';
import { Show, onMount } from 'solid-js';
import { serverData } from '../serverData';
import { connectWebSocket } from '../ws';
import { initDemoMode } from '../demo';
import { HostsTab } from './HostsTab';
import { ClientsTab } from './ClientsTab';
import { AuthWarningPanel } from './AuthWarningPanel';

/** Root component mounted into <div id="app"> inside <main>. */
export const App: Component = () => {
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

            <HostsTab configPath={serverData.configPath} />
            <ClientsTab configPath={serverData.configPath} />
        </>
    );
};
