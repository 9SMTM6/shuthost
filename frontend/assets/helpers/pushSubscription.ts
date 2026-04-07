import { apiFetch } from './apiFetch';

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

/**
 * Registers the service worker (if not already registered) and returns the
 * push subscription, creating one if it doesn't exist.
 */
const getOrCreatePushSubscription = async () => {
    if (!('serviceWorker' in navigator) || !('PushManager' in window)) {
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

    const existing = await registration.pushManager.getSubscription();
    if (existing) {
        return existing;
    }

    return registration.pushManager.subscribe({
        userVisibleOnly: true,
        applicationServerKey: urlBase64ToUint8Array(publicKey),
    });
};

/**
 * Subscribes the current browser to push notifications for when the given
 * host comes online.
 */
export const subscribeToHostOnline = async (hostname: string) => {
    const subscription = await getOrCreatePushSubscription();
    const subJson = subscription.toJSON();

    await apiFetch('/api/push/subscribe-host-online', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ subscription: subJson, hostname }),
    });
};
