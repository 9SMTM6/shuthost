/// <reference types="vite/client" />

import './styles.tailwind.css';
import { MetaProvider } from '@solidjs/meta';
import { Route, Router } from '@solidjs/router';
import { render } from 'solid-js/web';
import { showJSError } from './components/JsErrorBox';
import { demoSubpath } from './helpers/demo';
import { AboutPage } from './pages/AboutPage';
import { App } from './pages/App';
import { LoginPage } from './pages/LoginPage';
import { NotFoundPage } from './pages/NotFoundPage';

const appMount = document.getElementById('app');
if (appMount) {
    appMount.replaceChildren();
    render(
        () => (
            <MetaProvider>
                <Router base={demoSubpath}>
                    <Route path="/" component={App} />
                    <Route path="/login" component={LoginPage} />
                    <Route path="/about" component={AboutPage} />
                    <Route path="*" component={NotFoundPage} />
                </Router>
            </MetaProvider>
        ),
        appMount,
    );
}

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
