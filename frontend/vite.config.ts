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
                // CSS from the entry is renamed to styles.css so the Rust build
                // script can find it by the expected name.
                assetFileNames: (assetInfo) => {
                    const name = assetInfo.name ?? '';
                    if (name.endsWith('.css')) return 'styles[extname]';
                    return '[name][extname]';
                },
            },
        },
    },
});
