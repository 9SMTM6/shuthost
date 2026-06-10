import { Check, Copy } from 'lucide-solid';
import { createSignal, onCleanup } from 'solid-js';
import type { AnyComponent } from '../helpers/utils/solid';

export const CopyButton = ((props: { targetId: string; label: string }) => {
    const [text, setText] = createSignal('Copy');
    let timeout: ReturnType<typeof setTimeout> | undefined;

    onCleanup(() => {
        if (timeout) clearTimeout(timeout);
    });

    const handleClick = () => {
        const content = document.getElementById(props.targetId)?.textContent;
        if (!content) return;
        navigator.clipboard.writeText(content).then(() => {
            setText('Copied!');
            if (timeout) clearTimeout(timeout);
            timeout = setTimeout(() => setText('Copy'), 1500);
        });
    };

    const copied = () => text() === 'Copied!';

    return (
        <button
            class="copy-button"
            type="button"
            aria-label={copied() ? 'Copied!' : props.label}
            title={copied() ? 'Copied!' : props.label}
            onClick={handleClick}
        >
            {copied() ? (
                <Check size={14} aria-hidden="true" />
            ) : (
                <Copy size={14} aria-hidden="true" />
            )}
        </button>
    );
}) satisfies AnyComponent;
