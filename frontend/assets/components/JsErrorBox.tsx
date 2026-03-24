import type { Component } from 'solid-js';

/** Inline error banner shown by the global error handlers in index.tsx. */
export const JsErrorBox: Component = () => (
    <div id="js-error" class="alert alert-error mb-4" role="alert" hidden>
        <strong class="alert-title">JavaScript Error</strong>
        <p id="js-error-message" />
    </div>
);
