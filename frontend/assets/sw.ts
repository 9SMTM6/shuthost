/// <reference lib="webworker" />

// ShutHost service worker.
// Handles push notifications, notification click events, and asset caching.
// Registered eagerly at app startup so it is available for future use cases.

const sw = self as unknown as ServiceWorkerGlobalScope;

// Cache version: bump when making breaking cache-layout changes.
const CACHE_NAME = 'shuthost-v1';

// Hashed assets have exactly 8 lowercase hex chars between two dots (e.g. /app.a1b2c3d4.js).
// They are immutable, so a cache-first strategy is safe.
const isHashedAsset = (url: URL) => /\.[0-9a-f]{8}\.[a-z]+$/.test(url.pathname);

// Returns a base key for a hashed pathname by replacing the hash with a placeholder,
// used to identify and evict stale versions of the same asset.
// e.g. "/app.a1b2c3d4.js" → "/app..js"
const hashedBase = (pathname: string) =>
    pathname.replace(/\.[0-9a-f]{8}(\.[a-z]+)$/, '..$1');

// Deletes any cached entries that share the same base name as `url` but have a different hash.
const evictOldVersions = async (cache: Cache, url: URL) => {
    const base = hashedBase(url.pathname);
    const keys = await cache.keys();
    await Promise.all(
        keys
            .filter((req) => {
                const u = new URL(req.url);
                return (
                    u.origin === url.origin &&
                    u.pathname !== url.pathname &&
                    hashedBase(u.pathname) === base
                );
            })
            .map((req) => cache.delete(req)),
    );
};

// Scans freshly-fetched HTML for hashed asset URLs and prefetches any that are not yet cached.
// Relative URLs in the HTML are resolved against `pageUrl`.
const prefetchHashedAssets = async (
    cache: Cache,
    html: string,
    pageUrl: string,
) => {
    const base = new URL(pageUrl);
    const hrefs = [
        ...html.matchAll(/(?:href|src)="([^"]*\.[0-9a-f]{8}\.[a-z]+)"/g),
    ].flatMap((m) => (m[1] !== undefined ? [new URL(m[1], base).href] : []));

    await Promise.all(
        hrefs.map(async (href) => {
            const assetUrl = new URL(href);
            if (assetUrl.origin !== sw.location.origin) return;
            const req = new Request(href);
            if (await cache.match(req)) return; // already cached
            const response = await fetch(req).catch(() => null);
            if (response?.ok) {
                await evictOldVersions(cache, assetUrl);
                await cache.put(req, response);
            }
        }),
    );
};

// Returns the cached response immediately (cache-first).
// On a cache miss it fetches, evicts any older-hash version, and caches the result.
const cacheFirst = async (request: Request, url: URL) => {
    const cache = await caches.open(CACHE_NAME);
    const cached = await cache.match(request);
    if (cached) return cached;
    const response = await fetch(request);
    if (response.ok) {
        await evictOldVersions(cache, url);
        await cache.put(request, response.clone());
    }
    return response;
};

// Posts a NEW_VERSION_AVAILABLE message to all open window clients.
const notifyClients = async () => {
    const clients = await sw.clients.matchAll({ type: 'window' });
    for (const client of clients) {
        client.postMessage({ type: 'NEW_VERSION_AVAILABLE' });
    }
};

// Stale-while-revalidate for non-hashed assets (HTML pages, sw.js, etc.).
//
// responsePromise: serves the cached response immediately, or fetches on first load.
// bgWorkPromise:   when a cached version existed, revalidates in the background;
//                  if the HTML changed, prefetches newly referenced hashed assets
//                  and posts NEW_VERSION_AVAILABLE to all window clients.
const staleWhileRevalidate = (
    request: Request,
): {
    responsePromise: Promise<Response>;
    bgWorkPromise: Promise<void>;
} => {
    // Serve from cache immediately; populate cache on first load.
    const responsePromise = (async (): Promise<Response> => {
        const cache = await caches.open(CACHE_NAME);
        const cached = await cache.match(request);
        if (cached) return cached;
        const response = await fetch(request);
        if (response.ok) await cache.put(request, response.clone());
        return response;
    })();

    // Revalidate + diff in the background (skip on first load).
    const bgWorkPromise = (async (): Promise<void> => {
        const cache = await caches.open(CACHE_NAME);
        // Independent cache.match — each call returns a fresh Response handle.
        const cachedEntry = await cache.match(request);
        if (!cachedEntry) {
            // First load: responsePromise handles caching; nothing to compare yet.
            await responsePromise;
            return;
        }
        const cachedText = await cachedEntry.text();

        // Wait until the stale response has been handed to the browser.
        await responsePromise;

        const response = await fetch(request);
        if (!response.ok) return;

        const ct = response.headers.get('content-type') ?? '';
        if (ct.includes('text/html')) {
            const freshText = await response.text();
            await cache.put(
                request,
                new Response(freshText, {
                    status: response.status,
                    headers: { 'content-type': ct },
                }),
            );
            await prefetchHashedAssets(cache, freshText, request.url);
            if (freshText !== cachedText) {
                await notifyClients();
            }
        } else {
            await cache.put(request, response);
        }
    })();

    return {
        responsePromise,
        bgWorkPromise: bgWorkPromise
            .then(() => undefined)
            .catch(() => undefined),
    };
};

sw.addEventListener('fetch', (event) => {
    const url = new URL(event.request.url);

    // Only handle same-origin GET requests; leave API calls and other methods alone.
    if (event.request.method !== 'GET' || url.origin !== sw.location.origin)
        return;
    if (url.pathname.startsWith('/api/')) return;

    if (isHashedAsset(url)) {
        event.respondWith(cacheFirst(event.request, url));
    } else {
        const { responsePromise, bgWorkPromise } = staleWhileRevalidate(
            event.request,
        );
        event.respondWith(responsePromise);
        event.waitUntil(bgWorkPromise); // keep SW alive until revalidation + prefetch finish
    }
});

// Skip the waiting phase so the new SW activates immediately after install,
// rather than waiting for all tabs to close.
sw.addEventListener('install', (event) => {
    event.waitUntil(sw.skipWaiting());
});

// Delete caches from previous versions on activation, then take control immediately.
sw.addEventListener('activate', (event) => {
    event.waitUntil(
        caches
            .keys()
            .then((names) =>
                Promise.all(
                    names
                        .filter((n) => n !== CACHE_NAME)
                        .map((n) => caches.delete(n)),
                ),
            )
            .then(() => sw.clients.claim()),
    );
});

type PushPayload = {
    title: string;
    body: string;
    data?: Record<string, unknown>;
};

sw.addEventListener('push', (event) => {
    const payload: PushPayload = event.data?.json() ?? {
        title: 'ShutHost',
        body: 'A host changed state',
    };

    event.waitUntil(
        sw.registration.showNotification(payload.title, {
            body: payload.body,
            icon: '/favicon.svg',
            data: payload.data,
        }),
    );
});

sw.addEventListener('notificationclick', (event) => {
    event.notification.close();

    event.waitUntil(
        sw.clients
            .matchAll({ type: 'window', includeUncontrolled: true })
            .then((clientList) => {
                for (const client of clientList) {
                    if ('focus' in client) {
                        return client.focus();
                    }
                }
                if (sw.clients.openWindow) {
                    return sw.clients.openWindow('/');
                }
                return undefined;
            }),
    );
});
