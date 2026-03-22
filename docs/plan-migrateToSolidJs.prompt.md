# Plan: Migrate Frontend to SolidJS

## Summary
Replace the vanilla TypeScript `app.ts` (1500 LOC, procedural DOM manipulation) with SolidJS reactive components. Switch the build tool from `tsc + tailwind CLI` to Vite. Keep the three-page structure; only the dashboard gets SolidJS. **JS stays inlined** (Vite outputs fixed-name `app.js`; Rust reads and inlines it exactly as today; CSP hash machinery unchanged). **Tailwind** is fixed to scan HTML templates via `@source` directives. **Runtime server data** (config path, logout visibility, auth warning, demo mode) is consolidated from 4 separate HTML injections into a single JSON island that SolidJS reads on init.

---

## Phase 1 — Tooling & Build Setup

1. **Update `frontend/package.json`**
   - Add `dependencies`: `solid-js`
   - Add `devDependencies`: `vite`, `vite-plugin-solid`, `@tailwindcss/vite`
   - Replace `"build"` script: `"build": "vite build"` (single command replaces separate tsc + tailwind steps)
   - Keep `"build:diagrams"`, `"generate-npm-licenses"`, `"test"` unchanged
   - Remove `@tailwindcss/cli` (replaced by `@tailwindcss/vite`)

2. **Create `frontend/vite.config.ts`**
   - Plugins: `solid()` (vite-plugin-solid), `tailwindcss()` (@tailwindcss/vite)
   - `build.rollupOptions.input`: `{ app: 'assets/index.tsx' }` — single entry; login has no SolidJS
   - `build.rollupOptions.output.entryFileNames`: `'[name].js'` — **no hash in filename** (JS is inlined by Rust; cache-busting comes from the HTML page itself)
   - `build.rollupOptions.output.assetFileNames`: `'[name][extname]'` — also fixed names for CSS
   - `build.outDir: 'assets/generated'`
   - `build.emptyOutDir: false` (preserve icons, SVGs, other Rust-generated assets)

3. **Update `frontend/assets/styles.tailwind.css`** (the Tailwind CSS entry)
   - Add `@source` directives so Tailwind scans HTML template files too:
     ```css
     @import "tailwindcss";
     @source "../../**/*.html";
     @source "../../**/*.tmpl.html";
     @source "./**/*.hbs";
     ```
   - This ensures login.tmpl.html, about.tmpl.hbs, and all partials are scanned for class names

4. **Update `frontend/tsconfig.json`**
   - Add `"jsx": "preserve"` and `"jsxImportSource": "solid-js"` to `compilerOptions`
   - Ensure `"include"` covers `**/*.tsx`

## Phase 2 — Rust Build Script Updates (minimal changes)

5. **Update `coordinator/build/mod.rs`**
   - Remove separate `npm::run("build:tsc")` + `npm::run("build:tailwind")` calls
   - Replace with single `npm::run("build")`
   - Update `rerun-if-changed`: `app.ts` → `app.tsx`; keep tailwind-related reruns (Vite now handles both)
   - Everything else (icons, licenses, templates, CSP) unchanged

6. **`coordinator/build/templates.rs` — minimal update**
   - The `{ js }` substitution in `process_templates()` keeps exactly the same logic: read `../frontend/assets/generated/app.js` and inline it
   - No Vite manifest parsing needed (fixed output filenames, no hashes in filenames)
   - CSS filename is still `styles.css` (fixed), hash still computed from content → no change to `styles_hash` logic
   - No changes to `ASSET_HASH_*` env vars

7. **`coordinator/build/csp.rs` — no change**
   - The SolidJS bundle is still inlined as `<script type="module">{ js }</script>`
   - The Rust CSP hash is still computed from that inline block, exactly as today
   - Login inline script and external_auth_config hashing unchanged

## Phase 3 — Runtime Data: Replace 4 String Substitutions with One JSON Island

**Currently**, `render_ui_html()` in `coordinator/src/http/assets.rs` does 4 runtime string substitutions into `index.html`:
- `{ coordinator_config }` → config file path string
- `{ maybe_auth_warning }` → full HTML panel or empty
- `{ maybe_logout }` → logout form HTML or empty
- `{ maybe_demo_disclaimer }` → demo disclaimer HTML with `data-subpath`, or empty

**New approach**: consolidate into a single `<script id="server-data" type="application/json">` island that Rust injects. SolidJS reads it on init. This eliminates all injected HTML fragments.

8. **Add `{ server_data }` placeholder to `frontend/assets/index.tmpl.html`**
   - Just before `<div id="app">`: `<script id="server-data" type="application/json">{ server_data }</script>`
   - Remove the 4 old placeholders (`{ coordinator_config }`, `{ maybe_auth_warning }`, `{ maybe_logout }`, `{ maybe_demo_disclaimer }`) from template and header partial

9. **Update `render_ui_html()` in `coordinator/src/http/assets.rs`**
   - Instead of 4 `.replace(...)` calls, build a JSON object:
     ```json
     {"configPath":"...","showLogout":true,"authWarning":false,"isDemo":false,"demoSubpath":""}
     ```
   - One `.replace("{ server_data }", &json)` call
   - Use `serde_json::json!` macro (or manual string formatting, `serde_json` is likely already a dep)
   - Remove all 4 old replace calls from `render_ui_html`
   - `UiMode` enum can be simplified or kept as-is

## Phase 4 — SolidJS App Implementation

10. **Create `frontend/assets/index.tsx`** — Vite entry point
    - `import './styles.tailwind.css'` — triggers Vite CSS extraction
    - Read the `#server-data` JSON island: `const serverData = JSON.parse(document.getElementById('server-data').textContent)`
    - Pass as props or put in a module-level store
    - `render(() => <App serverData={...} />, document.getElementById('app'))`

11. **Create `frontend/assets/serverData.ts`**
    - `type ServerData = { configPath: string; showLogout: boolean; authWarning: boolean; isDemo: boolean; demoSubpath: string; }`
    - Export `const serverData: ServerData = JSON.parse(document.getElementById('server-data')!.textContent!)`
    - Imported by any component that needs it (config path display, logout button, auth warning panel, demo mode)

12. **Create `frontend/assets/stores/appStore.ts`**
    - SolidJS `createStore` for: `hosts`, `statusMap`, `leaseMap`, `clientList`, `clientStats`
    - Expose setters called by WebSocket message handlers

13. **Create `frontend/assets/ws.ts`** — WebSocket logic
    - Connect to `/ws`, handle reconnect, bfcache
    - On each message type (`Initial`, `HostStatus`, `ConfigChanged`, `LeaseUpdate`): update the store

14. **Create `frontend/assets/demo.ts`** — demo mode
    - Detect via `serverData.isDemo`
    - Mock WebSocket using `serverData.demoSubpath`
    - Feeds same message shapes as real backend into store setters

15. **Create `frontend/assets/components/App.tsx`**
    - `onMount`: if `serverData.isDemo` → `initDemoMode()`, else `initWebSocket()`
    - `<Show when={serverData.authWarning}>` → `<AuthWarningPanel />`
    - `<Show when={serverData.showLogout}>` → `<LogoutButton />` (renders `<form action="/logout" method="post">`)
    - Renders `<HostsTable />`, `<ClientsTable />`, `<InstallSection configPath={serverData.configPath} />`

16. **Create `frontend/assets/components/HostsTable.tsx`** + **`HostRow.tsx`**
    - `<For each={hosts()}>`, status badges, lease actions, API calls

17. **Create `frontend/assets/components/ClientsTable.tsx`** + **`ClientRow.tsx`**
    - `<For each={clients()}>`, last-seen, reset button

18. **Create `frontend/assets/components/InstallSection.tsx`**
    - Receives `configPath` prop; renders install commands + copy-to-clipboard

19. **Create `frontend/assets/components/AuthWarningPanel.tsx`**
    - Renders the security warning currently in `partials/external_auth_config.tmpl.html`
    - Shown only when `serverData.authWarning === true`
    - This replaces the injected HTML fragment; CSP hash for this partial can be removed

20. **Update `frontend/assets/index.tmpl.html`**
    - Add `<div id="app"></div>` mount point (main content area)
    - Add `<script id="server-data" type="application/json">{ server_data }</script>` island
    - Keep `<script type="module">{ js }</script>` (still inlined by Rust build script)
    - Remove the old `{ coordinator_config }`, `{ maybe_auth_warning }`, `{ maybe_logout }`, `{ maybe_demo_disclaimer }` placeholders from template and `header.tmpl.html`

21. **Update `frontend/assets/partials/header.tmpl.html`**
    - Remove `{ maybe_demo_disclaimer }` and `{ maybe_logout }` placeholders (now rendered by SolidJS)
    - The header HTML shell (nav tabs, logo, hamburger) can remain as static HTML for fast initial render; SolidJS `<App>` is mounted alongside it, appending logout/demo banner dynamically

## Verification

1. `cargo build -p shuthost_coordinator` — build succeeds; Vite runs, templates processed, CSP hashes computed
2. `just playwright --reporter=line` — full E2E suite
3. Browser: check WebSocket updates, lease actions, install section, config path display
4. Browser console: no errors, no CSP violations
5. Demo mode on GitHub Pages: visit demo URL, simulated data loads correctly
6. `just pixelpeep` — visual regression

---

## Decisions

- **JS stays inline**: Vite outputs `app.js` (fixed filename, no hash); Rust reads and inlines it as before; existing CSP hash machinery unchanged. No new asset route needed.
- **Tailwind on login/about**: Fix via `@source` directives in `styles.tailwind.css` — Tailwind scans all `.html`/`.tmpl.html` files.
- **Runtime data via JSON island**: 4 string substitutions → 1 JSON object. SolidJS reads it on init; no injected HTML fragments.
- **Auth warning moves to a SolidJS component**: `AuthWarningPanel.tsx` replaces the injected `external_auth_config.tmpl.html` partial. Its CSP hash entry in `csp.rs` can be removed.
- **Login page**: No SolidJS; existing 10-line inline toggle script unchanged.
- **About page**: No SolidJS; fully static.
- **Out of scope**: routing, SSR, new features.

## Key Files

- `frontend/package.json` — deps update, single build script
- `frontend/vite.config.ts` — new file
- `frontend/tsconfig.json` — JSX config
- `frontend/assets/styles.tailwind.css` — add `@source` directives
- `frontend/assets/index.tmpl.html` — add mount point, JSON island, remove old placeholders
- `frontend/assets/partials/header.tmpl.html` — remove `{ maybe_logout }`, `{ maybe_demo_disclaimer }`
- `frontend/assets/app.ts` — deleted after migration
- `frontend/assets/index.tsx` — new Vite entry
- `frontend/assets/serverData.ts` — typed JSON island reader
- `frontend/assets/components/*.tsx` — new components
- `frontend/assets/stores/appStore.ts` — SolidJS state
- `frontend/assets/ws.ts` — WebSocket logic
- `frontend/assets/demo.ts` — demo mode
- `coordinator/build/mod.rs` — single `npm run build` call, update reruns
- `coordinator/src/http/assets.rs` — `render_ui_html` outputs one JSON replace instead of 4 HTML replaces
