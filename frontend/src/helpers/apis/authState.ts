import { createSignal } from 'solid-js';
import { serverData } from '../dataIslands';
import { demoSubpath, isDemoMode } from '../demo';

export type AuthStatus = 'probing' | 'authenticated' | 'unauthenticated';

// In demo mode there is no real backend — treat the user as authenticated.
const needsProbe =
    !isDemoMode &&
    (serverData.authMode === 'token' || serverData.authMode === 'oidc');

const [authStatus, setAuthStatus] = createSignal<AuthStatus>(
    needsProbe ? 'probing' : 'authenticated',
);

if (needsProbe) {
    fetch(`${demoSubpath}/api/hosts_status`, {
        method: 'HEAD',
        credentials: 'same-origin',
    })
        .then((res) => {
            if (res.status === 401) {
                console.warn(
                    `Auth probe: received ${res.status} (expected for unauthenticated users)`,
                );
                setAuthStatus('unauthenticated');
            } else {
                setAuthStatus('authenticated');
            }
        })
        .catch(() => {
            /* leave 'probing' on network error */
        });
}

export { authStatus };
