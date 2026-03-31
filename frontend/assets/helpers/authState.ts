import { createSignal } from 'solid-js';
import { demoSubpath, isDemoMode } from './demo';
import { serverData } from './serverData';

type AuthStatus = 'unknown' | 'yes' | 'no';

// In demo mode there is no real backend — treat the user as authenticated.
const needsProbe =
    !isDemoMode &&
    (serverData.authMode === 'token' || serverData.authMode === 'oidc');

const [_authStatus, setAuthStatus] = createSignal<AuthStatus>(
    needsProbe ? 'unknown' : 'yes',
);

if (needsProbe) {
    fetch(`${demoSubpath}/api/hosts_status`, {
        method: 'HEAD',
        credentials: 'same-origin',
    })
        .then((res) => setAuthStatus(res.status === 401 ? 'no' : 'yes'))
        .catch(() => {
            /* leave 'unknown' on network error */
        });
}

/**
 * Reactive: `true` = logged in, `false` = not logged in, `undefined` = probe still in flight.
 */
export const isLoggedIn = (): boolean | undefined => {
    const s = _authStatus();
    if (s === 'unknown') return undefined;
    return s === 'yes';
};
