import { Title } from '@solidjs/meta';
import { useNavigate } from '@solidjs/router';
import type { ParentProps } from 'solid-js';
import { createEffect, onMount, Show } from 'solid-js';
import { authStatus } from '../helpers/authState';
import { initDemoMode, isDemoMode } from '../helpers/demo';
import { serverData } from '../helpers/serverData';
import type { AnyComponent } from '../helpers/solidUtils';
import { connectWebSocket } from '../helpers/websockerManagement';
import { AuthWarningPanel } from './AuthWarningPanel';
import { Footer } from './Footer';
import { Header } from './Header';
import { JsErrorBox } from './JsErrorBox';

export const AppLayout = ((props: ParentProps) => {
    const navigate = useNavigate();
    createEffect(() => {
        if (authStatus() === 'unauthenticated') {
            navigate('/login', { replace: true });
        }
    });
    onMount(() => {
        if (isDemoMode) return initDemoMode();
        connectWebSocket();
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
