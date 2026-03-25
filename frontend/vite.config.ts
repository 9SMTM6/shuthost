import { defineConfig } from 'vite';
import solid from 'vite-plugin-solid';
import tailwindcss from '@tailwindcss/vite';
import { readFileSync } from 'fs';
import { resolve } from 'path';

const buildData = JSON.parse(
    readFileSync(resolve(__dirname, 'assets/generated/build-data.json'), 'utf-8')
) as { repository: string };

export default defineConfig({
    define: {
        __BUILD_REPOSITORY__: JSON.stringify(buildData.repository),
    },
    plugins: [
        solid(),
        tailwindcss(),
    ],
    build: {
        outDir: 'assets/generated',
        emptyOutDir: false,
        rolldownOptions: {
            input: {
                app: 'assets/index.tsx',
            },
            output: {
                // No hash in filenames: Rust reads app.js and inlines it, so
                // cache-busting comes from the CSP hash of the inline block.
                entryFileNames: '[name].js',
                assetFileNames: '[name][extname]',
            },
        },
        target: 'esnext',
    },
});
