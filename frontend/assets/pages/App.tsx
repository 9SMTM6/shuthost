import { Title } from '@solidjs/meta';
import { onMount, Show } from 'solid-js';
import { ArchitectureTab } from '../components/ArchitectureTab';
import { AuthWarningPanel } from '../components/AuthWarningPanel';
import { ClientsTab } from '../components/ClientsTab';
import { Footer } from '../components/Footer';
import { Header } from '../components/Header';
import { HostsTab } from '../components/HostsTab';
import { JsErrorBox } from '../components/JsErrorBox';
import type { AnyComponent } from '../helpers/component';
import { initDemoMode, isDemoMode } from '../helpers/demo';
import { serverData } from '../helpers/serverData';
import { connectWebSocket } from '../helpers/ws';

export const App = (() => {
    onMount(() => {
        if (isDemoMode) {
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
            <Footer />
        </>
    );
}) satisfies AnyComponent;
