import { buildData } from '../helpers/buildData';
import type { AnyComponent } from '../helpers/component';

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

/** Inline error banner shown by the global error handlers in index.tsx. */
export const JsErrorBox = (() => (
    <div id="js-error" class="alert alert-error mb-4" role="alert" hidden>
        <strong class="alert-title">JavaScript Error</strong>
        <p id="js-error-message" />
        <p>
            <a
                id="js-error-issue-link"
                href={`${buildData.repository}/issues`}
                target="_blank"
                rel="external noopener noreferrer"
                class="underline text-sm"
            >
                Report this issue on GitHub
            </a>
        </p>
    </div>
)) satisfies AnyComponent;
