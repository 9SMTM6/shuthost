// DO NOT import any solidjs (including indirectly via e.g. components) in this file.

export const showJSError = (message: string) => {
    const errorDiv = document.getElementById(
        'js-error',
    ) as HTMLDivElement | null;
    const messageEl = document.getElementById(
        'js-error-message',
    ) as HTMLParagraphElement | null;
    if (errorDiv && messageEl) {
        messageEl.textContent = message;
        errorDiv.hidden = false;
    }
};

export const safeExternalUrl = (href: string): string => {
    const trimmed = href.trim();
    if (!/^https?:\/\//i.test(trimmed)) {
        console.error(`Rejected unsafe external URL: ${href}`);
        return '#';
    }
    try {
        const url = new URL(trimmed);
        if (['https:', 'http:'].includes(url.protocol)) {
            return url.href;
        }
    } catch {
        // fall through
    }
    console.error(`Rejected invalid external URL: ${href}`);
    return '#';
};

const RTF = new Intl.RelativeTimeFormat(undefined, { numeric: 'auto' });

export const formatRelativeTimestamp = (
    isoTimestamp: string | null | undefined,
    now = Date.now(),
): string => {
    if (!isoTimestamp) return 'Never';
    const date = new Date(isoTimestamp);
    const diffMs = now - date.getTime();
    const oneYearMs = 365 * 24 * 60 * 60 * 1000;
    if (diffMs >= oneYearMs) return date.toLocaleString();
    const seconds = Math.round(diffMs / 1000);
    if (seconds < 45) return 'just now';
    if (seconds < 90) return RTF.format(-1, 'minute');
    const minutes = Math.round(seconds / 60);
    if (minutes < 60) return RTF.format(-minutes, 'minute');
    const hours = Math.round(minutes / 60);
    if (hours < 24) return RTF.format(-hours, 'hour');
    const days = Math.round(hours / 24);
    if (days < 7) return RTF.format(-days, 'day');
    if (days < 30) return RTF.format(-Math.round(days / 7), 'week');
    const months = Math.round(days / 30);
    if (months < 12) return RTF.format(-months, 'month');
    return date.toLocaleString();
};

export const sortActiveFirst = <T>(
    items: T[],
    isActive: (item: T) => boolean,
    getName: (item: T) => string,
) => {
    const compare = (a: T, b: T) => getName(a).localeCompare(getName(b));
    return [
        ...items.filter(isActive).toSorted(compare),
        ...items.filter((i) => !isActive(i)).toSorted(compare),
    ];
};

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
    } = {}
) => {
    // default to checking resp.ok and redirecting on 401.
    // We dont assign these in a default object parameter because then they would be overridden when fetch options are passed in.
    const {
        checkRespOk = true, checkAndRedirectUnauthorized = true, ...fetchInit
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
                err instanceof Error ? err.message : 'Unknown fetch error'
            );
        }
        throw err;
    }
};

