import { buildData } from '../helpers/buildData';
import { safeExternalUrl } from '../helpers/utils';
import type { AnyComponent } from '../helpers/utils';

/** Inline error banner shown by the global error handlers in index.tsx. */
export const JsErrorBox = (() => (
    <div id="js-error" class="alert alert-error mb-4" role="alert" hidden>
        <strong class="alert-title">JavaScript Error</strong>
        <p id="js-error-message" />
        <p>
            <a
                id="js-error-issue-link"
                href={safeExternalUrl(`${buildData.repository}/issues`)}
                target="_blank"
                rel="external noopener noreferrer"
                class="underline text-sm"
            >
                Report this issue on GitHub
            </a>
        </p>
    </div>
)) satisfies AnyComponent;
