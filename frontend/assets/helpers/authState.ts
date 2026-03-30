import { createSignal } from 'solid-js';
import { serverData } from './serverData';

type AuthStatus = 'unknown' | 'yes' | 'no';

// For auth modes that require no login, we are always considered authenticated.
const needsProbe = serverData.authMode === 'token' || serverData.authMode === 'oidc';

const [_authStatus, setAuthStatus] = createSignal<AuthStatus>(needsProbe ? 'unknown' : 'yes');

if (needsProbe) {
    fetch('/api/hosts_status', { method: 'HEAD', credentials: 'same-origin' })
        .then(res => setAuthStatus(res.status === 401 ? 'no' : 'yes'))
        .catch(() => { /* leave 'unknown' on network error */ });
}

/**
 * Reactive: `true` = logged in, `false` = not logged in, `undefined` = probe still in flight.
 */
export const isLoggedIn = (): boolean | undefined => {
    const s = _authStatus();
    if (s === 'unknown') return undefined;
    return s === 'yes';
};
