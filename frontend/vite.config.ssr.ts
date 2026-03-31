import { defineConfig } from 'vite';
import solid from 'vite-plugin-solid';

// Minimal config used only for the prerender step (npm run prerender via vite-node).
// SSR mode enables SolidJS's server-side renderToString transform.
export default defineConfig({
    plugins: [solid({ ssr: true })],
});
