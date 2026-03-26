/// <reference types="vite/client" />

import './styles.tailwind.css';
import { render } from 'solid-js/web';
import { Router, Route } from '@solidjs/router';
import type { RouteSectionProps } from '@solidjs/router';
import { Header } from './components/Header';
import { App } from './pages/App';
import { LoginPage } from './pages/LoginPage';
import { JsErrorBox, showJSError } from './components/JsErrorBox';

// TODO:
// * make index a index.html instead of index.tsx
// * revalidate the static rendering approach

const RootLayout = (props: RouteSectionProps) => (
    <>
        <Header />
        <main
            id="main-content"
            class="main px-4 sm:px-6 lg:px-8 max-w-7xl mx-auto w-full"
            tabindex="-1"
        >
            <section class="py-4 sm:py-6">
                <JsErrorBox />
                {props.children}
            </section>
        </main>
    </>
);

const appMount = document.getElementById('app');
if (appMount) {
    render(() => (
        <Router root={RootLayout}>
            <Route path="/" component={App} />
            <Route path="/login" component={LoginPage} />
        </Router>
    ), appMount);
}

// Global error handlers
window.addEventListener('error', (event) => {
    console.error('Global error:', event.error);
    const message = event.error?.message || 'An unknown error occurred';
    showJSError(message);
});

window.addEventListener('unhandledrejection', (event) => {
    console.error('Unhandled promise rejection:', event.reason);
    const message = event.reason?.message || 'An unhandled promise rejection occurred';
    showJSError(message);
});

window.addEventListener('securitypolicyviolation', (event) => {
    // Ignore violations originating from browser extensions — they inject their
    // own styles/scripts and are correctly blocked by our CSP, but are not our bug.
    if (event.sourceFile?.startsWith('moz-extension://') || event.sourceFile?.startsWith('chrome-extension://')) {
        console.warn('CSP violation from browser extension (ignored):', event.sourceFile);
        return;
    }
    console.error('Security policy violation:', event);
    showJSError('A security policy violation occurred');
});
