import { defineConfig } from 'vite';
import solid from 'vite-plugin-solid';

// Minimal config used only for the prerender step (npm run prerender via vite-node).
// SSR mode enables SolidJS's server-side renderToString transform.
// ssr.noExternal prevents Node's native ESM loader from choking on .jsx files
// shipped by @solidjs/router (exposed directly via pnpm's strict symlink layout).
export default defineConfig({
    plugins: [solid({ ssr: true })],
    ssr: {
        noExternal: [/@solidjs\//],
    },
});
