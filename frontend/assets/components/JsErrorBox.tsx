import type { Component } from 'solid-js';

declare const __BUILD_REPOSITORY__: string;

/** Inline error banner shown by the global error handlers in index.tsx. */
export const JsErrorBox = (() => (
    <div id="js-error" class="alert alert-error mb-4" role="alert" hidden>
        <strong class="alert-title">JavaScript Error</strong>
        <p id="js-error-message" />
        <p>
            {/* Shown only for non-network errors, href is set dynamically by the global error handlers */}
            <a
                id="js-error-issue-link"
                href={`${__BUILD_REPOSITORY__}/issues`}
                target="_blank"
                rel="noopener noreferrer"
                class="underline text-sm"
            >
                Report this issue on GitHub
            </a>
        </p>
    </div>
)) satisfies Component<any>;
