import './styles.tailwind.css';
import { render } from 'solid-js/web';
import { App } from './components/App';
import { LogoutButton, DemoDisclaimer } from './components/HeaderSlots';

// Mount the main app
const appMount = document.getElementById('app');
if (appMount) render(() => <App />, appMount);

// Mount the logout button into its slot in the static header
const logoutMount = document.getElementById('logout-mount');
if (logoutMount) render(() => <LogoutButton />, logoutMount);

// Mount the demo disclaimer into its slot in the static header
const demoBannerMount = document.getElementById('demo-banner-mount');
if (demoBannerMount) render(() => <DemoDisclaimer />, demoBannerMount);

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
