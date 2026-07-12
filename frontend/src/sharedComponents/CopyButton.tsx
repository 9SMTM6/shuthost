import { Check, Copy } from 'lucide-solid';
import { createSignal, onCleanup } from 'solid-js';
import type { JSX } from 'solid-js/h/jsx-runtime';
import { serverData } from '../helpers/dataIslands';
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

export type CopyableCodeBlockProps = {
    id: string;
    value: string;
    label?: string;
    classList?: JSX.ClassList;
};

export const CopyableCodeBlock = ((props: CopyableCodeBlockProps) => (
    <div class="code-container" classList={props.classList}>
        <CopyButton targetId={props.id} label={props.label ?? 'Copy'} />
        <code
            id={props.id}
            class="code-block"
            // add a data-attribute that identifies config location copy blocks. This is used to fix up (redact) the config location in snapshots.
            {...(props.value === serverData.configPath
                ? { 'data-config-location': '' }
                : {})}
        >
            {props.value}
        </code>
    </div>
)) satisfies AnyComponent;

export type CopyableInstallCommandProps = {
    id: string;
    title: string;
    command: string;
};

export const CopyableInstallCommand = ((props: CopyableInstallCommandProps) => (
    <>
        <p class="mb-1 text-xs font-semibold">{props.title}</p>
        <CopyableCodeBlock
            id={props.id}
            value={props.command}
            label="Copy install command"
            classList={{ 'py-2': true }}
        />
    </>
)) satisfies AnyComponent;
