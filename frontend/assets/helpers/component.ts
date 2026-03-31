import type { Component } from 'solid-js';

// biome-ignore lint/suspicious/noExplicitAny: intentional type alias so call sites don't need per-site suppressions
export type AnyComponent = Component<any>;
