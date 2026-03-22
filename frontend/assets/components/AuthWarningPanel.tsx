import type { Component } from 'solid-js';
import { onMount } from 'solid-js';

/**
 * Security warning panel for when no internal auth is configured or the
 * external auth exceptions_version is outdated. Replaces the injected
 * `external_auth_config.tmpl.html` partial.
 */
export const AuthWarningPanel: Component = () => {
    onMount(() => {
        const baseUrl = window.location.origin;
        const domain = baseUrl.replace(/^https?:\/\//, '');

        const autheliaConfig = document.getElementById('authelia-config');
        const traefikConfig = document.getElementById('traefik-config');

        if (autheliaConfig) {
            autheliaConfig.textContent =
                `- domain: ${domain}\n    policy: bypass\n    resources:\n        - '^/download/(.*)'\n        - '^/api/m2m/(.*)$'\n        - '/manifest\\..*\\.json$'\n        - '/favicon\\..*\\.svg$'`;
        }
        if (traefikConfig) {
            traefikConfig.textContent =
                `# Add to your service labels\n- "traefik.http.routers.shuthost-bypass.rule=Host(\`${domain}\`) && (PathPrefix(\`/download\`) || PathPrefix(\`/api/m2m\`) || PathRegexp(\`/manifest\\..*\\.json\`) || PathRegexp(\`/favicon\\..*\\.svg\`))"\n- "traefik.http.routers.shuthost-bypass.priority=100"\n# Remove auth middleware for bypass routes`;
        }
    });

    return (
        <section class="section-container mb-4" aria-labelledby="security-config-title">
            <div class="alert alert-warning">
                <div class="alert-title">Important: No Internal Authentication, or outdated External Auth config</div>
                <p class="mb-1">
                    If you do not configure external authentication (reverse proxy auth) or an internal authentication
                    mode (Token or OIDC), the web UI will allow any visitor to switch hosts on or off. This is a
                    security risk.
                </p>
                <p class="text-sm font-semibold mt-2">Options:</p>
                <ul>
                    <li>Enable the built-in Token (easy to configure) or OIDC authentication in the coordinator config.</li>
                    <li>
                        Use an external authentication gateway (reverse proxy) and set the{' '}
                        <code>exceptions_version</code> in your auth config to acknowledge you have configured the
                        required bypass rules.
                    </li>
                </ul>
                <p class="text-xs mt-2">
                    <em>
                        When using an external auth reverse proxy, you must allow unauthenticated access to certain
                        endpoints so installers and machine-to-machine clients can work. See the examples below.
                    </em>
                </p>
            </div>

            <details class="collapsible-details" aria-labelledby="security-config-title">
                <summary
                    class="collapsible-header collapsed"
                    aria-controls="security-config-content"
                    id="security-config-header"
                >
                    <h2 class="section-title mb-0 text-base" id="security-config-title">
                        🔒 Required Security Exceptions
                    </h2>
                    <span class="collapsible-icon" aria-hidden="true" />
                </summary>
                <div
                    id="security-config-content"
                    class="collapsible-content collapsed"
                    role="region"
                    aria-labelledby="security-config-title"
                >
                    <div class="alert alert-info">
                        <div class="alert-title">Authentication Bypass Required</div>
                        <p>
                            For the web app and installation scripts to work properly, these endpoints must be reachable
                            without authentication. If your reverse proxy enforces authentication (Authelia, NPM with
                            auth, etc.), create bypass rules for:
                        </p>
                        <ul>
                            <li><code>/download/*</code> — Installation script and binary downloads</li>
                            <li><code>/api/m2m/*</code> — Machine-to-machine API communication used by agents/clients</li>
                            <li><code>/manifest.*.json</code> — PWA manifest required for webpage installability</li>
                            <li><code>/favicon.*.svg</code> — Favicon required for browser installability</li>
                        </ul>
                        <p class="text-xs mt-2">
                            <em>
                                These exceptions let installer scripts and the web UI fetch resources and allow agent
                                clients to call machine-to-machine APIs without user login. The manifest and favicon
                                entries are necessary for correct browser/PWA behaviour.
                            </em>
                        </p>
                    </div>

                    <div class="alert alert-warning">
                        <div class="alert-title">Configuration Examples</div>
                        <p class="text-sm font-semibold mb-2">Authelia:</p>
                        <div class="code-container">
                            <button class="copy-button" data-copy-target="#authelia-config" type="button" aria-label="Copy Authelia config">Copy</button>
                            <code id="authelia-config" class="code-block">Loading...</code>
                        </div>

                        <p class="text-sm font-semibold mb-2 mt-4">Nginx Proxy Manager with Authentication:</p>
                        <div class="code-container">
                            <button class="copy-button" data-copy-target="#nginx-config" type="button" aria-label="Copy Nginx config">Copy</button>
                            <code id="nginx-config" class="code-block">{`# In your proxy host's advanced configuration
location ~ ^/(download|api/m2m|manifest\\..*\\.json|favicon\\..*\\.svg)$ {
    auth_basic off;
    proxy_pass http://your-shuthost-backend;
}`}</code>
                        </div>

                        <p class="text-sm font-semibold mb-2 mt-4">Traefik with ForwardAuth:</p>
                        <div class="code-container">
                            <button class="copy-button" data-copy-target="#traefik-config" type="button" aria-label="Copy Traefik config">Copy</button>
                            <code id="traefik-config" class="code-block">Loading...</code>
                        </div>

                        <p class="text-xs mt-2">
                            <em>
                                Replace backend references with your actual configuration values. After configuring your
                                proxy rules, set{' '}
                                <code>{'auth = { type = "external", exceptions_version = 1 }'}</code>{' '}
                                in the coordinator config to acknowledge the exceptions. If this doesn't help, please
                                raise an issue.
                            </em>
                        </p>
                    </div>
                </div>
            </details>
        </section>
    );
};
