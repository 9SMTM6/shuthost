import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';
import solid from 'vite-plugin-solid';

// In dev mode, the Header renders <img src="/favicon.{hash}.svg"> using the hash from build-data.
// The dev index.html uses hash "dev", so the request is for /favicon.dev.svg.
// This plugin intercepts those requests and serves the real asset instead.
const hashedFaviconRedirect = {
    name: 'dev-hashed-favicon-redirect',
    configureServer(server: import('vite').ViteDevServer) {
        server.middlewares.use((req, _res, next) => {
            if (req.url?.match(/^\/favicon\.[^.]+\.svg(\?.*)?$/)) {
                req.url = '/assets/favicon.svg';
            }
            next();
        });
    },
};

export default defineConfig({
    plugins: [solid(), tailwindcss(), hashedFaviconRedirect],
    server: {
        open: true,
    },
    build: {
        outDir: 'assets/generated',
        emptyOutDir: false,
        rolldownOptions: {
            input: {
                app: 'assets/index.tsx',
                sw: 'assets/sw.ts',
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
