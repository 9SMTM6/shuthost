/**
 * Vite configuration used only by `vite-node generate-pages.tsx`.
 * Enables SolidJS SSR transforms so renderToString() works correctly.
 */

import { defineConfig } from 'vite';
import solid from 'vite-plugin-solid';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
    plugins: [
        solid({ ssr: true }),
        tailwindcss(),
    ],
});
