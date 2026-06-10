import { showJSError } from './utils';

export class ApiFetchUnauthorizedError extends Error {
    constructor() {
        super('Unauthorized');
        this.name = 'ApiFetchUnauthorizedError';
        if (typeof Error.captureStackTrace === 'function') {
            Error.captureStackTrace(this, ApiFetchUnauthorizedError);
        }
    }
}

export const apiFetch = async (
    url: string,
    options: RequestInit & {
        checkRespOk?: boolean;
        checkAndRedirectUnauthorized?: boolean;
    } = {},
) => {
    // default to checking resp.ok and redirecting on 401.
    // We dont assign these in a default object parameter because then they would be overridden when fetch options are passed in.
    const {
        checkRespOk = true,
        checkAndRedirectUnauthorized = true,
        ...fetchInit
    } = options;

    try {
        const resp = await fetch(url, fetchInit);
        if (checkAndRedirectUnauthorized && resp.status === 401) {
            window.location.assign('/login');
            throw new ApiFetchUnauthorizedError();
        }
        if (checkRespOk && !resp.ok && resp.status !== 401) {
            const msg = `HTTP ${resp.status}: ${resp.statusText}`;
            showJSError(msg);
            throw new Error(msg);
        }
        return resp;
    } catch (err) {
        if (!(err instanceof ApiFetchUnauthorizedError)) {
            showJSError(
                err instanceof Error ? err.message : 'Unknown fetch error',
            );
        }
        throw err;
    }
};
