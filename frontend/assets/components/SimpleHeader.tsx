import type { Component } from 'solid-js';

/** Minimal header for static/about pages. No router dependency — safe to use in SSR (generate-pages). */
export const SimpleHeader: Component = () => (
    <header class="bg-white dark:bg-[#1e1e1e] shadow-md" role="banner">
        <div class="max-w-full mx-auto px-4 sm:px-6 lg:px-8">
            <div class="flex items-center h-(--header-height)">
                <a href="/" class="flex items-center gap-4">
                    <img src="/favicon.svg" alt="ShutHost Logo" class="h-6 sm:h-8 w-auto" />
                    <h1 class="text-xl sm:text-2xl font-semibold text-black dark:text-[#cccccc]">
                        ShutHost
                    </h1>
                </a>
            </div>
        </div>
    </header>
);
