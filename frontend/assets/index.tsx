/// <reference types="vite/client" />

import './styles.tailwind.css';
import { MetaProvider } from '@solidjs/meta';
import { Navigate, Route, Router } from '@solidjs/router';
import { createSignal, Show } from 'solid-js';
import { render } from 'solid-js/web';
import { showJSError } from './components/JsErrorBox';
import { demoSubpath } from './helpers/demo';
import {
    onUpdateAvailable,
    registerServiceWorker,
} from './helpers/serviceWorker';
import { AboutPage } from './pages/AboutPage';
import { ArchitecturePage } from './pages/Architecture';
import { ClientsPage } from './pages/Clients';
import { HostDetailPage } from './pages/HostDetailPage';
import { HostsPage } from './pages/Hosts';
import { LoginPage } from './pages/LoginPage';
import { NotFoundPage } from './pages/NotFoundPage';

const [updateAvailable, setUpdateAvailable] = createSignal(false);

const appMount = document.getElementById('app');
if (appMount) {
    appMount.replaceChildren();
    render(
        () => (
            <MetaProvider>
                <Router base={demoSubpath}>
                    <Route
                        path="/"
                        component={() => <Navigate href="/hosts" />}
                    />
                    <Route path="/hosts" component={HostsPage} />
                    <Route path="/clients" component={ClientsPage} />
                    <Route path="/docs" component={ArchitecturePage} />
                    <Route path="/login" component={LoginPage} />
                    <Route path="/about" component={AboutPage} />
                    <Route path="/hosts/:hostname" component={HostDetailPage} />
                    <Route path="*" component={NotFoundPage} />
                </Router>
                <Show when={updateAvailable()}>
                    <div class="fixed bottom-4 right-4 z-50 flex items-center gap-3 rounded-lg border-l-4 border-[#64b5f6] bg-[#d8f3ff] px-4 py-3 text-[#005fb8] shadow-lg dark:border-[#0078d4] dark:bg-[rgba(0,120,212,0.10)] dark:text-[#4fc3f7]">
                        <span class="text-sm">A new version is available.</span>
                        <button
                            type="button"
                            class="rounded bg-[#005fb8] px-2 py-1 text-xs font-semibold text-white hover:bg-[#004e9a] dark:bg-[#0078d4] dark:hover:bg-[#006cbd]"
                            onClick={() => location.reload()}
                        >
                            Refresh
                        </button>
                    </div>
                </Show>
            </MetaProvider>
        ),
        appMount,
    );
}

registerServiceWorker();
onUpdateAvailable(() => setUpdateAvailable(true));

// Global error handlers
window.addEventListener('error', (event) => {
    console.error('Global error:', event.error);
    const message = event.error?.message || 'An unknown error occurred';
    showJSError(message);
});

window.addEventListener('unhandledrejection', (event) => {
    console.error('Unhandled promise rejection:', event.reason);
    const message =
        event.reason?.message || 'An unhandled promise rejection occurred';
    showJSError(message);
});

window.addEventListener('securitypolicyviolation', (event) => {
    // Ignore violations originating from browser extensions — they inject their
    // own styles/scripts and are correctly blocked by our CSP, but are not our bug.
    if (
        event.sourceFile?.startsWith('moz-extension://') ||
        event.sourceFile?.startsWith('chrome-extension://')
    ) {
        console.warn(
            'CSP violation from browser extension (ignored):',
            event.sourceFile,
        );
        return;
    }
    console.error('Security policy violation:', event);
    showJSError('A security policy violation occurred');
});
