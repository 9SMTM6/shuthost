import { Title } from '@solidjs/meta';
import type { ParentProps } from 'solid-js';
import { onMount, Show } from 'solid-js';
import type { AnyComponent } from '../helpers/utils';
import { initDemoMode, isDemoMode } from '../helpers/demo';
import { serverData } from '../helpers/serverData';
import { connectWebSocket } from '../helpers/ws';
import { AuthWarningPanel } from './AuthWarningPanel';
import { Footer } from './Footer';
import { Header } from './Header';
import { JsErrorBox } from './JsErrorBox';

export const AppLayout = ((props: ParentProps) => {
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
                    {props.children}
                </section>
            </main>
            <Footer />
        </>
    );
}) satisfies AnyComponent;
