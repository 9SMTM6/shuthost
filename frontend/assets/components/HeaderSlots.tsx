import type { Component } from 'solid-js';
import { Show } from 'solid-js';
import { serverData } from '../serverData';
import { demoSubpath } from '../demo';

/** Rendered into the logout slot inside the static header. */
export const LogoutButton: Component = () => (
    <Show when={serverData.showLogout}>
        <form method="post" action="/logout">
            <button
                type="submit"
                class="text-xs sm:text-sm px-3 py-1 rounded border border-transparent btn btn-red"
                aria-label="Logout"
            >
                Logout
            </button>
        </form>
    </Show>
);

/** Rendered before the header content (inside the header element). */
export const DemoDisclaimer: Component = () => (
    <Show when={serverData.isDemo}>
        <div
            id="demo-mode-disclaimer"
            data-subpath={demoSubpath}
            class="w-full bg-[#fff8e1] dark:bg-[rgba(204,167,0,0.15)] text-[#bf8803] dark:text-[#cca700] border border-[#ffd54f] dark:border-[#8a7300] px-4 py-2 text-center font-semibold"
        >
            Demo Mode: Static UI with simulated interactions only
        </div>
    </Show>
);
