import { showJSError } from '../components/JsErrorBox';

export const apiFetch = async (
    url: string,
    options?: RequestInit,
): Promise<Response> => {
    try {
        const resp = await fetch(url, options);
        if (resp.status === 401) {
            window.location.assign('/login');
            throw new Error('Unauthorized');
        }
        if (!resp.ok) {
            const msg = `HTTP ${resp.status}: ${resp.statusText}`;
            showJSError(msg);
            throw new Error(msg);
        }
        return resp;
    } catch (err) {
        if (!(err instanceof Error && err.message === 'Unauthorized')) {
            showJSError(
                err instanceof Error ? err.message : 'Unknown fetch error',
            );
        }
        throw err;
    }
};
