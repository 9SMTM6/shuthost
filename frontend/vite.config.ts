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
        rolldownOptions: {
            input: {
                app: 'assets/index.tsx',
            },
            output: {
                // No hash in filenames: Rust reads app.js serves it under hashed url.
                // Since we dont clear the out-dir, we'd leave back stale files if we keep the hash in the name.
                entryFileNames: '[name].js',
                assetFileNames: '[name][extname]',
            },
        },
        target: 'esnext',
    },
});
