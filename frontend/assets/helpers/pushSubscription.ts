import { apiFetch } from './apiFetch';
import { demo, isDemoMode } from './demo';
import { noServiceWorkerSupport } from './serviceWorker';

/**
 * Converts a URL-safe base64 string (no padding) to a Uint8Array.
 * Required for passing the VAPID public key to PushManager.subscribe().
 */
const urlBase64ToUint8Array = (base64: string) => {
    const padding = '='.repeat((4 - (base64.length % 4)) % 4);
    const base64Std = (base64 + padding).replace(/-/g, '+').replace(/_/g, '/');
    const rawData = atob(base64Std);
    const buffer = new ArrayBuffer(rawData.length);
    const output = new Uint8Array(buffer);
    for (let i = 0; i < rawData.length; i++) {
        output[i] = rawData.charCodeAt(i);
    }
    return output;
};

const noPushSupport = noServiceWorkerSupport || !('PushManager' in window);

/**
 * Registers the service worker (if not already registered) and returns the
 * push subscription, creating one if it doesn't exist.
 */
const getOrCreatePushSubscription = async () => {
    if (noPushSupport) {
        throw new Error('Push notifications are not supported in this browser');
    }

    const permission = await Notification.requestPermission();
    if (permission !== 'granted') {
        throw new Error('Notification permission denied');
    }

    // The service worker is registered eagerly at app startup; just wait for it.
    const registration = await navigator.serviceWorker.ready;

    const vapidResp = await fetch('/api/push/vapid-public-key');
    if (!vapidResp.ok) {
        throw new Error('Failed to fetch VAPID public key');
    }
    const { publicKey } = (await vapidResp.json()) as { publicKey: string };

    const applicationServerKey = urlBase64ToUint8Array(publicKey);

    const existing = await registration.pushManager.getSubscription();
    if (existing) {
        // The existing subscription may have been created with a different VAPID
        // key (e.g. after a server restart that regenerated the key).  If the
        // keys don't match the push service will reject future sends, so
        // unsubscribe first and fall through to create a fresh subscription.
        const existingKey = existing.options.applicationServerKey;
        if (existingKey) {
            const existingBytes = new Uint8Array(existingKey);
            const keysMatch =
                existingBytes.length === applicationServerKey.length &&
                existingBytes.every((b, i) => b === applicationServerKey[i]);
            if (keysMatch) {
                return existing;
            }
        }
        await existing.unsubscribe();
    }

    return registration.pushManager.subscribe({
        userVisibleOnly: true,
        applicationServerKey,
    });
};

/**
 * Subscribes the current browser to push notifications for unscheduled events
 * on the given host (startup or shutdown not triggered by ShutHost).
 */
export const subscribeToHostUnscheduled = async (hostname: string) => {
    if (isDemoMode) return demo.subscribeToHostUnscheduled(hostname);
    const subscription = await getOrCreatePushSubscription();
    const subJson = subscription.toJSON();

    await apiFetch('/api/push/subscribe-host-unscheduled', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ subscription: subJson, hostname }),
    });
};

/**
 * Checks whether the current browser is already subscribed to unscheduled-event
 * push notifications for the given host. Does NOT request notification permission.
 * Returns false if the browser has no push subscription or push is unsupported.
 */
export const checkHostUnscheduledSubscription = async (hostname: string) => {
    if (isDemoMode) return demo.checkHostUnscheduledSubscription(hostname);
    if (noPushSupport) {
        return false;
    }

    const registration = await navigator.serviceWorker.ready;
    const existing = await registration.pushManager.getSubscription();
    if (!existing) return false;

    const endpoint = encodeURIComponent(existing.endpoint);
    const resp = await fetch(
        `/api/push/subscribe-host-unscheduled?endpoint=${endpoint}&hostname=${encodeURIComponent(hostname)}`,
    );
    if (!resp.ok) return false;
    const { subscribed } = (await resp.json()) as { subscribed: boolean };
    return subscribed;
};

/**
 * Removes the unscheduled-event subscription link for the current browser + host pair.
 * Has no effect if the browser has no push subscription.
 */
export const unsubscribeFromHostUnscheduled = async (hostname: string) => {
    if (isDemoMode) return demo.unsubscribeFromHostUnscheduled(hostname);
    if (noPushSupport) return;

    const registration = await navigator.serviceWorker.ready;
    const existing = await registration.pushManager.getSubscription();
    if (!existing) return;

    await apiFetch('/api/push/subscribe-host-unscheduled', {
        method: 'DELETE',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ endpoint: existing.endpoint, hostname }),
    });
};

/**
 * Subscribes the current browser to push notifications for operation-failed events
 * on the given host (when a shutdown or startup command times out or errors).
 */
export const subscribeToHostOperationFailed = async (hostname: string) => {
    if (isDemoMode) return demo.subscribeToHostOperationFailed(hostname);
    const subscription = await getOrCreatePushSubscription();
    const subJson = subscription.toJSON();

    await apiFetch('/api/push/subscribe-host-operation-failed', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ subscription: subJson, hostname }),
    });
};

/**
 * Checks whether the current browser is already subscribed to operation-failed
 * push notifications for the given host.
 */
export const checkHostOperationFailedSubscription = async (
    hostname: string,
) => {
    if (isDemoMode) return demo.checkHostOperationFailedSubscription(hostname);
    if (noPushSupport) {
        return false;
    }

    const registration = await navigator.serviceWorker.ready;
    const existing = await registration.pushManager.getSubscription();
    if (!existing) return false;

    const endpoint = encodeURIComponent(existing.endpoint);
    const resp = await fetch(
        `/api/push/subscribe-host-operation-failed?endpoint=${endpoint}&hostname=${encodeURIComponent(hostname)}`,
    );
    if (!resp.ok) return false;
    const { subscribed } = (await resp.json()) as { subscribed: boolean };
    return subscribed;
};

/**
 * Removes the operation-failed subscription link for the current browser + host pair.
 */
export const unsubscribeFromHostOperationFailed = async (hostname: string) => {
    if (isDemoMode) return demo.unsubscribeFromHostOperationFailed(hostname);
    if (noPushSupport) return;

    const registration = await navigator.serviceWorker.ready;
    const existing = await registration.pushManager.getSubscription();
    if (!existing) return;

    await apiFetch('/api/push/subscribe-host-operation-failed', {
        method: 'DELETE',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ endpoint: existing.endpoint, hostname }),
    });
};

/**
 * Subscribes the current browser to recurring push notifications when the given
 * host has been online for `durationSecs` seconds. Replaces any existing
 * subscription for this host with the new duration.
 */
export const subscribeToHostOnlineFor = async (
    hostname: string,
    durationSecs: number,
) => {
    if (isDemoMode)
        return demo.subscribeToHostOnlineFor(hostname, durationSecs);
    const subscription = await getOrCreatePushSubscription();
    const subJson = subscription.toJSON();

    await apiFetch('/api/push/subscribe-host-online-for', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
            subscription: subJson,
            hostname,
            duration_secs: durationSecs,
        }),
    });
};

/**
 * Checks whether the current browser is subscribed to recurring online-for
 * notifications for the given host. Returns the duration in seconds if subscribed,
 * or `null` if not.
 */
export const checkHostOnlineForSubscription = async (hostname: string) => {
    if (isDemoMode) return demo.checkHostOnlineForSubscription(hostname);
    if (noPushSupport) {
        return null;
    }

    const registration = await navigator.serviceWorker.ready;
    const existing = await registration.pushManager.getSubscription();
    if (!existing) return null;

    const endpoint = encodeURIComponent(existing.endpoint);
    const resp = await fetch(
        `/api/push/subscribe-host-online-for?endpoint=${endpoint}&hostname=${encodeURIComponent(hostname)}`,
    );
    if (!resp.ok) return null;
    const data = (await resp.json()) as {
        subscribed: boolean;
        duration_secs?: number;
    };
    return data.subscribed && data.duration_secs != null
        ? data.duration_secs
        : null;
};

/**
 * Removes the recurring online-for subscription link for the current browser + host pair.
 */
export const unsubscribeFromHostOnlineFor = async (hostname: string) => {
    if (isDemoMode) return demo.unsubscribeFromHostOnlineFor(hostname);
    if (noPushSupport) return;

    const registration = await navigator.serviceWorker.ready;
    const existing = await registration.pushManager.getSubscription();
    if (!existing) return;

    await apiFetch('/api/push/subscribe-host-online-for', {
        method: 'DELETE',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ endpoint: existing.endpoint, hostname }),
    });
};

/**
 * Registers a one-shot push notification that fires once the given host has been
 * online for `durationSecs` seconds. The host must currently be online on the
 * coordinator; if not, throws an error.
 */
export const subscribeToHostOnlineForOneshot = async (
    hostname: string,
    durationSecs: number,
) => {
    if (isDemoMode)
        return demo.subscribeToHostOnlineForOneshot(hostname, durationSecs);
    const subscription = await getOrCreatePushSubscription();
    const subJson = subscription.toJSON();

    await apiFetch('/api/push/subscribe-host-online-for-oneshot', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
            subscription: subJson,
            hostname,
            duration_secs: durationSecs,
        }),
    });
};
