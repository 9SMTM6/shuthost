// DO NOT import any (other than type) parts from component files in this file.
import type { Component } from 'solid-js';

// biome-ignore lint/suspicious/noExplicitAny: intentional type alias so call sites don't need per-site suppressions
export type AnyComponent = Component<any>;

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
): string => {
    if (!isoTimestamp) return 'Never';
    const date = new Date(isoTimestamp);
    const diffMs = Date.now() - date.getTime();
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
): T[] => {
    const compare = (a: T, b: T) => getName(a).localeCompare(getName(b));
    return [
        ...items.filter(isActive).toSorted(compare),
        ...items.filter((i) => !isActive(i)).toSorted(compare),
    ];
};
