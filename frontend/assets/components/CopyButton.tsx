import type { Component } from 'solid-js';
import { createSignal, onCleanup } from 'solid-js';

export const CopyButton: Component<{ targetId: string; label: string }> = (props) => {
    const [text, setText] = createSignal('Copy');
    let timeout: ReturnType<typeof setTimeout> | undefined;

    onCleanup(() => { if (timeout) clearTimeout(timeout); });

    const handleClick = () => {
        const content = document.getElementById(props.targetId)?.textContent;
        if (!content) return;
        navigator.clipboard.writeText(content).then(() => {
            setText('Copied!');
            if (timeout) clearTimeout(timeout);
            timeout = setTimeout(() => setText('Copy'), 1500);
        });
    };

    return (
        <button class="copy-button" type="button" aria-label={props.label} onClick={handleClick}>
            {text()}
        </button>
    );
};
