import { defineConfig } from 'vite';
import solid from 'vite-plugin-solid';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
    plugins: [
        solid(),
        tailwindcss(),
    ],
    build: {
        outDir: 'assets/generated',
        emptyOutDir: false,
        rollupOptions: {
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
    },
});
