/// <reference types="vite/client" />

import './styles.tailwind.css';
import { render } from 'solid-js/web';
import { Router, Route } from '@solidjs/router';
import type { RouteSectionProps } from '@solidjs/router';
import { Header } from './components/Header';
import { App } from './components/App';
import { LoginPage } from './components/LoginPage';
import { JsErrorBox } from './components/JsErrorBox';

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

const showJsError = (message: string) => {
    const errorDiv = document.getElementById('js-error') as HTMLDivElement;
    const messageEl = document.getElementById('js-error-message') as HTMLParagraphElement;
    messageEl.textContent = message;
    errorDiv.hidden = false;
};

// Global error handlers
window.addEventListener('error', (event) => {
    console.error('Global error:', event.error);
    const message = event.error?.message || 'An unknown error occurred';
    showJsError(message);
});

window.addEventListener('unhandledrejection', (event) => {
    console.error('Unhandled promise rejection:', event.reason);
    const message = event.reason?.message || 'An unhandled promise rejection occurred';
    showJsError(message);
});

window.addEventListener('securitypolicyviolation', (event) => {
    // Ignore violations originating from browser extensions — they inject their
    // own styles/scripts and are correctly blocked by our CSP, but are not our bug.
    if (event.sourceFile?.startsWith('moz-extension://') || event.sourceFile?.startsWith('chrome-extension://')) {
        console.warn('CSP violation from browser extension (ignored):', event.sourceFile);
        return;
    }
    console.error('Security policy violation:', event);
    showJsError('A security policy violation occurred');
});
