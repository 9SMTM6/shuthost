import { createSignal, onCleanup } from 'solid-js';
import type { Component, ParentComponent } from 'solid-js';

// biome-ignore lint/suspicious/noExplicitAny: intentional type alias so call sites don't need per-site suppressions
export type AnyComponent = Component<any>;

// biome-ignore lint/suspicious/noExplicitAny: intentional type alias so call sites don't need per-site suppressions
export type AnyParentComponent = ParentComponent<any>;

/** Current time signal, updated at a regular interval */
export const useCurrentTime = (intervalMs = 60_000) => {
    const [currentTime, setCurrentTime] = createSignal(Date.now());
    const interval = window.setInterval(
        () => setCurrentTime(Date.now()),
        intervalMs,
    );
    onCleanup(() => window.clearInterval(interval));
    return currentTime;
};
