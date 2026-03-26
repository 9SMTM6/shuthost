import type { Component } from 'solid-js';

export const showJSError = (message: string) => {
    const errorDiv = document.getElementById('js-error') as HTMLDivElement | null;
    const messageEl = document.getElementById('js-error-message') as HTMLParagraphElement | null;
    if (errorDiv && messageEl) {
        messageEl.textContent = message;
        errorDiv.hidden = false;
    }
};

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
                rel="external noopener noreferrer"
                class="underline text-sm"
            >
                Report this issue on GitHub
            </a>
        </p>
    </div>
)) satisfies Component<any>;
