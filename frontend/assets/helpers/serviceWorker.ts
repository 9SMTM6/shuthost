import { demoSubpath } from './demo';

/**
 * Registers the service worker as early as possible.
 * Uses `demoSubpath` so that subpath deployments (e.g. GitHub Pages) register
 * at the correct URL (e.g. `/shuthost/sw.js` instead of `/sw.js`).
 * Safe to call multiple times — the browser deduplicates registrations for
 * the same script URL and scope.
 *
 * Returns the registration promise so callers can await the active worker if
 * needed, or ignore it for fire-and-forget registration.
 */
export const registerServiceWorker = () => {
    if (!('serviceWorker' in navigator)) {
        return null;
    }
    const swUrl = `${demoSubpath}/sw.js`;
    return navigator.serviceWorker.register(swUrl, { type: 'module' }).catch((err) => {
        console.warn('Service worker registration failed:', err);
    });
};

/**
 * Calls `callback` whenever the service worker signals that a new version of
 * the app is available (i.e. fresh HTML was fetched and differed from the cache).
 * The caller should prompt the user to reload.
 */
export const onUpdateAvailable = (callback: () => void) => {
    if (!('serviceWorker' in navigator)) return;
    navigator.serviceWorker.addEventListener(
        'message',
        (event: MessageEvent) => {
            if (
                (event.data as { type?: string } | null)?.type ===
                'NEW_VERSION_AVAILABLE'
            ) {
                callback();
            }
        },
    );
};
