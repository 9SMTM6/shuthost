/**
 * Registers /sw.js as a module service worker as early as possible.
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
    return navigator.serviceWorker.register('/sw.js', { type: 'module' });
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
