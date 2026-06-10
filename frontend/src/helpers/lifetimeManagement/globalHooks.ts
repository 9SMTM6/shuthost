import { showJSError } from '../utils';
import { connectWebSocket, closeWebSocket } from './websocket';

export const registerGlobalErrorHandlers = () => {
    window.addEventListener('error', (event) => {
        console.error('Global error:', event.error);
        const message = event.error?.message || 'An unknown error occurred';
        showJSError(message);
    });

    window.addEventListener('unhandledrejection', (event) => {
        console.error('Unhandled promise rejection:', event.reason);
        const message = event.reason?.message || 'An unhandled promise rejection occurred';
        showJSError(message);
    });

    window.addEventListener('securitypolicyviolation', (event) => {
        // Ignore violations originating from browser extensions — they inject their
        // own styles/scripts and are correctly blocked by our CSP, but are not our bug.
        if (event.sourceFile?.startsWith('moz-extension://') ||
            event.sourceFile?.startsWith('chrome-extension://')) {
            console.warn(
                'CSP violation from browser extension (ignored):',
                event.sourceFile
            );
            return;
        }
        console.error('Security policy violation:', event);
        showJSError('A security policy violation occurred');
    });
};

export const backForwardCacheHandling = () => {
    window.addEventListener('pageshow', (event) => {
        if (event.persisted) {
            console.info('Page restored from bfcache, reconnecting WebSocket');
            connectWebSocket();
        }
    });

    window.addEventListener('pagehide', (event) => {
        if (event.persisted) {
            console.info('Page being cached, closing WebSocket');
            closeWebSocket();
        }
    });
};
