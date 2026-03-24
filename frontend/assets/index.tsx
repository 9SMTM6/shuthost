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
            <JsErrorBox />
            <section class="py-4 sm:py-6">
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
    const errorDiv = document.getElementById('js-error') as HTMLDivElement | null;
    const messageEl = document.getElementById('js-error-message') as HTMLParagraphElement | null;
    if (errorDiv && messageEl) {
        messageEl.textContent = event.error?.message || 'An unknown error occurred';
        errorDiv.hidden = false;
    }
});

window.addEventListener('unhandledrejection', (event) => {
    console.error('Unhandled promise rejection:', event.reason);
    const errorDiv = document.getElementById('js-error') as HTMLDivElement | null;
    const messageEl = document.getElementById('js-error-message') as HTMLParagraphElement | null;
    if (errorDiv && messageEl) {
        messageEl.textContent = event.reason?.message || 'An unhandled promise rejection occurred';
        errorDiv.hidden = false;
    }
});

window.addEventListener('securitypolicyviolation', (event) => {
    console.error('Security policy violation:', event);
    const errorDiv = document.getElementById('js-error') as HTMLDivElement | null;
    const messageEl = document.getElementById('js-error-message') as HTMLParagraphElement | null;
    if (errorDiv && messageEl) {
        messageEl.textContent = 'A security policy violation occurred';
        errorDiv.hidden = false;
    }
});
