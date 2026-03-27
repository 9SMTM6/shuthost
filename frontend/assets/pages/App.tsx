import type { Component } from 'solid-js';
import { Show, onMount } from 'solid-js';
import { Title } from '@solidjs/meta';
import { serverData } from '../helpers/serverData';
import { connectWebSocket } from '../helpers/ws';
import { initDemoMode } from '../helpers/demo';
import { Header } from '../components/Header';
import { HostsTab } from '../components/HostsTab';
import { ClientsTab } from '../components/ClientsTab';
import { AuthWarningPanel } from '../components/AuthWarningPanel';
import { ArchitectureTab } from '../components/ArchitectureTab';
import { JsErrorBox } from '../components/JsErrorBox';

export const App = (() => {
    onMount(() => {
        if (serverData.demoSubpath) {
            initDemoMode();
        } else {
            connectWebSocket();
        }
    });

    return (
        <>
            <Title>ShutHost Coordinator</Title>
            <Header />
            <main
                id="main-content"
                class="main px-4 sm:px-6 lg:px-8 max-w-7xl mx-auto w-full"
                tabindex="-1"
            >
                <section class="py-4 sm:py-6">
                    <JsErrorBox />
                    {/* Auth security warning */}
                    <Show when={serverData.authWarning}>
                        <AuthWarningPanel />
                    </Show>

                    <ArchitectureTab />

                    <HostsTab configPath={serverData.configPath} />
                    <ClientsTab configPath={serverData.configPath} />
                </section>
            </main>
        </>
    );
}) satisfies Component<any>;
