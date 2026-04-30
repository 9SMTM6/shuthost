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

// Network-first with cache fallback, used for navigation requests (refreshes).
//
// networkRequest is the original browser navigate request; it preserves the real
// Sec-Fetch-Mode/Dest headers so the server handles it like a browser navigation
// (auth redirects, OIDC flows, etc.) rather than a sub-resource fetch.
//
// cacheKey is the canonical URL used for cache storage and lookup; all SPA routes
// are stored under a single entry (the scope root) to avoid cache fragmentation.
//
// Falls back to the cached SPA shell when the network returns a non-ok response
// or throws; the SPA then handles auth/routing itself.
const networkFirstWithCacheFallback = async (
    networkRequest: Request,
    cacheKey: Request,
): Promise<Response> => {
    const cache = await caches.open(CACHE_NAME);
    let networkResponse: Response | null = null;
    try {
        networkResponse = await fetch(networkRequest);
        if (networkResponse.ok) {
            await cache.put(cacheKey, networkResponse.clone());
            return networkResponse;
        }
    } catch {
        // network failure – fall through to cache
    }
    const cached = await cache.match(cacheKey);
    if (cached) return cached;
    // No cached copy: return whatever the network gave us (or an error response).
    return networkResponse ?? Response.error();
};

// How long to wait for the network before falling back to the cached response.
const REVALIDATE_TIMEOUT_MS = 250;

// Fastest-of strategy for non-hashed assets: races cache vs network with a timeout deadline.
//
// responsePromise: serves whichever of cache (hit) or network responds first.
//                  If the timeout fires before the network, falls back to the cached response
//                  (or waits for the network on a cache miss).
// bgWorkPromise:   if a cached response was served, keeps the in-flight network request alive
//                  to revalidate in the background; if the HTML changed, prefetches newly
//                  referenced hashed assets and posts NEW_VERSION_AVAILABLE to all clients.
const fastestOfWithTimeout = (
    request: Request,
): { responsePromise: Promise<Response>; bgWorkPromise: Promise<void> } => {
    // Fire network fetch immediately so it runs in parallel with cache lookup.
    const networkFetchPromise = fetch(request.clone());
    const cacheMatchPromise = caches
        .open(CACHE_NAME)
        .then((c) => c.match(request));
    let servedFromCache = false;

    const responsePromise = (async () => {
        const cache = await caches.open(CACHE_NAME);

        // Only resolves on a cache hit; never resolves on a miss so it drops
        // out of the race and network or timeout can win instead.
        const cacheHit = new Promise<Response>((resolve) => {
            cacheMatchPromise
                .then((r) => {
                    if (r) resolve(r);
                })
                .catch(() => {});
        });

        const first = await Promise.race([
            cacheHit.then((r) => ({ from: 'cache' as const, r })),
            networkFetchPromise.then((r) => ({ from: 'network' as const, r })),
            new Promise<{ from: 'timeout' }>((resolve) => {
                setTimeout(
                    () => resolve({ from: 'timeout' }),
                    REVALIDATE_TIMEOUT_MS,
                );
            }),
        ]);

        if (first.from === 'network') {
            if (first.r.ok) await cache.put(request.clone(), first.r.clone());
            return first.r;
        }

        if (first.from === 'cache') {
            servedFromCache = true;
            return first.r;
        }

        // Timeout fired: use cache if available, otherwise wait for network.
        const cached = await cacheMatchPromise;
        if (cached) {
            servedFromCache = true;
            return cached;
        }

        const response = await networkFetchPromise;
        if (response.ok) await cache.put(request.clone(), response.clone());
        return response;
    })();

    const bgWorkPromise = (async () => {
        await responsePromise;
        if (!servedFromCache) return;

        // We served a stale cached response; revalidate with the in-flight network request.
        const cache = await caches.open(CACHE_NAME);
        // Independent cache.match — each call returns a fresh Response handle.
        const cachedEntry = await cache.match(request);
        const cachedText = cachedEntry ? await cachedEntry.text() : null;

        const networkResponse = await networkFetchPromise.catch(() => null);
        if (!networkResponse?.ok) return;

        const ct = networkResponse.headers.get('content-type') ?? '';
        if (ct.includes('text/html')) {
            const freshText = await networkResponse.text();
            await cache.put(
                request,
                new Response(freshText, {
                    status: networkResponse.status,
                    headers: { 'content-type': ct },
                }),
            );
            await prefetchHashedAssets(cache, freshText, request.url);
            if (freshText !== cachedText) await notifyClients();
        } else {
            await cache.put(request, networkResponse);
        }
    })();

    return {
        responsePromise,
        bgWorkPromise: bgWorkPromise.then(() => undefined).catch(() => undefined),
    };
};

sw.addEventListener('fetch', (event) => {
    const url = new URL(event.request.url);

    // Only handle same-origin GET requests; leave API calls, auth flows, and other methods alone.
    if (event.request.method !== 'GET' || url.origin !== sw.location.origin)
        return;
    const scopePath = new URL(sw.registration.scope).pathname;
    if (
        url.pathname.startsWith(`${scopePath}api/`) ||
        url.pathname.startsWith(`${scopePath}oidc/`)
    ) {
        return;
    }

    // Navigation requests (SPA page loads) all return the same HTML; canonicalize
    // to the scope root so we only keep one cached HTML entry instead of one per route.
    // On refresh: network-first (using the real navigate request so the server sees
    // Sec-Fetch-Mode: navigate), cache fallback.
    if (event.request.mode === 'navigate') {
        const cacheKey = new Request(sw.registration.scope);
        event.respondWith(
            networkFirstWithCacheFallback(event.request, cacheKey),
        );
        return;
    }

    if (isHashedAsset(url)) {
        event.respondWith(cacheFirst(event.request, url));
    } else {
        const { responsePromise, bgWorkPromise } = fastestOfWithTimeout(
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
            icon: new URL('favicon.svg', sw.registration.scope).href,
            data: payload.data,
        }),
    );
});

sw.addEventListener('notificationclick', (event) => {
    event.notification.close();

    const hostname = event.notification.data?.['hostname'];
    const targetUrl = new URL(
        hostname ? `/hosts/${encodeURIComponent(String(hostname))}` : '/',
        sw.registration.scope,
    ).href;

    event.waitUntil(
        sw.clients
            .matchAll({ type: 'window', includeUncontrolled: true })
            .then(async (clientList) => {
                for (const windowClient of clientList) {
                    if (windowClient.url !== targetUrl) {
                        await windowClient.navigate?.(targetUrl);
                    }
                    return windowClient.focus();
                }
                return sw.clients.openWindow(targetUrl);
            }),
    );
});
