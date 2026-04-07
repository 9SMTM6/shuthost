/// <reference lib="webworker" />

// ShutHost service worker.
// Currently handles push notifications and notification click events.
// Registered eagerly at app startup so it is available for future use cases.

const sw = self as unknown as ServiceWorkerGlobalScope;

type PushPayload = {
    title: string;
    body: string;
    data?: Record<string, unknown>;
}

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
