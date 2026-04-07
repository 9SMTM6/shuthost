/**
 * Registers /sw.js as a module service worker as early as possible.
 * Safe to call multiple times — the browser deduplicates registrations for
 * the same script URL and scope.
 *
 * Returns the registration promise so callers can await the active worker if
 * needed, or ignore it for fire-and-forget registration.
 */
export const registerServiceWorker = (): Promise<ServiceWorkerRegistration> | null => {
    if (!('serviceWorker' in navigator)) {
        return null;
    }
    return navigator.serviceWorker.register('/sw.js', { type: 'module' });
};
