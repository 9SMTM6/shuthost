// Service Worker for handling push notifications

declare var self: ServiceWorkerGlobalScope;
export { };

// Different types of push notifications
// To add a new notification type:
// 1. Add a new union member to NotificationType
// 2. Create a corresponding data interface (e.g., NewTypeData)
// 3. Update the Rust NotificationType enum in notifications.rs
// 4. Add handling logic in the push event listener if needed
type NotificationType =
| { type: 'host_status'; data: HostStatusData };

// Data for host status notifications
type HostStatusData = {
    host: string;
    action: 'online' | 'offline';
}

self.addEventListener('install', () => {
    console.info('Service Worker installing.');
    // Skip waiting to activate immediately
    self.skipWaiting();
});

self.addEventListener('activate', (event) => {
    console.info('Service Worker activating.');
    // Claim all clients to start controlling them immediately
    event.waitUntil(self.clients.claim());
});

self.addEventListener('push', (event) => {
    console.info('Push message received:', event);
    
    if (!event.data) {
        console.info('Push message has no data');
        return;
    }
    
    const notification = event.data.json() as NotificationType;
    console.info('Push data:', notification);
    
    // Generate title and options based on notification type
    let title: string;
    let options: NotificationOptions;
    
    switch (notification.type) {
        case 'host_status':
            title = 'Host Status';
            const { host, action } = notification.data;
            options = {
                body: `Host '${host}' is now ${action}`,
                icon: '{ favicon_src }',
                badge: '{ favicon_src }',
                tag: `host-${action}`,
                requireInteraction: false,
                silent: false,
                data: notification.data,
            };
            break;
        default:
            console.warn('Received unrecognized notification type:', notification.type);
            return; // Don't show notification for unrecognized types
    }
    
    event.waitUntil(
        self.registration.showNotification(title, options)
    );
});

self.addEventListener('notificationclick', (event) => {
    console.info('Notification click received:', event);
    
    event.notification.close();
    
    // Focus existing window or open new one
    event.waitUntil(
        self.clients.matchAll({ type: 'window', includeUncontrolled: true }).then((clientList: readonly WindowClient[]) => {
            const url = self.location.origin;
            
            for (const client of clientList) {
                if (client.url === url && 'focus' in client) {
                    return client.focus();
                }
            }
            
            if (self.clients.openWindow) {
                return self.clients.openWindow(url);
            }
            return Promise.resolve(null);
        })
    );
});