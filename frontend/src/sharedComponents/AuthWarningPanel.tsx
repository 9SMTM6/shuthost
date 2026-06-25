import type { AnyComponent } from '../helpers/utils/solid';
import { CopyableCodeBlock } from './CopyButton';

/**
 * Security warning panel for when no internal auth is configured or the
 * external auth exceptions_version is outdated.
 */
export const AuthWarningPanel = (() => {
    const domain = window.location.origin.replace(/^https?:\/\//, '');

    return (
        <section
            class="section-container mb-4"
            aria-labelledby="security-config-title"
        >
            <div class="alert alert-warning">
                <div class="alert-title">
                    Important: No Internal Authentication, or outdated External
                    Auth config
                </div>
                <p class="mb-1">
                    If you do not configure external authentication (reverse
                    proxy auth) or an internal authentication mode (Token or
                    OIDC), the web UI will allow any visitor to switch hosts on
                    or off. This is a security risk.
                </p>
                <p class="text-sm font-semibold mt-2">Options:</p>
                <ul>
                    <li>
                        Enable the built-in Token (easy to configure) or OIDC
                        authentication in the coordinator config.
                    </li>
                    <li>
                        Use an external authentication gateway (reverse proxy)
                        and set the <code>exceptions_version</code> in your auth
                        config to acknowledge you have configured the required
                        bypass rules.
                    </li>
                </ul>
                <p class="text-xs mt-2">
                    <em>
                        When using an external auth reverse proxy, you must
                        allow unauthenticated access to certain endpoints so
                        installers and machine-to-machine clients can work. See
                        the examples below.
                    </em>
                </p>
            </div>

            <details
                class="collapsible-details"
                aria-labelledby="security-config-title"
            >
                <summary
                    class="collapsible-header collapsed"
                    aria-controls="security-config-content"
                    id="security-config-header"
                >
                    <h2
                        class="section-title mb-0 text-base"
                        id="security-config-title"
                    >
                        🔒 Required Security Exceptions
                    </h2>
                    <span class="collapsible-icon" aria-hidden="true" />
                </summary>
                <section
                    id="security-config-content"
                    class="collapsible-content collapsed"
                    aria-labelledby="security-config-title"
                >
                    <div class="alert alert-info">
                        <div class="alert-title">
                            Authentication Bypass Required
                        </div>
                        <p>
                            For the web app and installation scripts to work
                            properly, these endpoints must be reachable without
                            authentication. If your reverse proxy enforces
                            authentication (Authelia, NPM with auth, etc.),
                            create bypass rules for:
                        </p>
                        <ul>
                            <li>
                                <code>/download/*</code> — Installation script
                                and binary downloads
                            </li>
                            <li>
                                <code>/api/m2m/*</code> — Machine-to-machine API
                                communication used by agents/clients
                            </li>
                            <li>
                                <code>/manifest.*.json</code> — PWA manifest
                                required for webpage installability
                            </li>
                            <li>
                                <code>/favicon.*.svg</code> — Favicon required
                                for browser installability
                            </li>
                        </ul>
                        <p class="text-xs mt-2">
                            <em>
                                These exceptions let installer scripts and the
                                web UI fetch resources and allow agent clients
                                to call machine-to-machine APIs without user
                                login. The manifest and favicon entries are
                                necessary for correct browser/PWA behaviour.
                            </em>
                        </p>
                    </div>

                    <div class="alert alert-warning">
                        <div class="alert-title">Configuration Examples</div>
                        <p class="text-sm font-semibold mb-2">Authelia:</p>
                        <CopyableCodeBlock
                            label="Copy Authelia config"
                            id="authelia-config"
                            value={`- domain: ${domain}
    policy: bypass
    resources:
        - '^/download/(.*)'
        - '^/api/m2m/(.*)$'
        - '/manifest..*.json$'
        - '/favicon..*.svg$'`}
                        />

                        <p class="text-sm font-semibold mb-2 mt-4">
                            Nginx Proxy Manager with Authentication:
                        </p>
                        <CopyableCodeBlock
                            label="Copy Nginx config"
                            id="nginx-config"
                            value={`# In your proxy host's advanced configuration
location ~ ^/(download|api/m2m|manifest\\..*\\.json|favicon\\..*\\.svg)$ {
    auth_basic off;
    proxy_pass http://your-shuthost-backend;
}`}
                        />

                        <p class="text-sm font-semibold mb-2 mt-4">
                            Traefik with ForwardAuth:
                        </p>
                        <CopyableCodeBlock
                            label="Copy Traefik config"
                            id="traefik-config"
                            value={`# Add to your service labels
- "traefik.http.routers.shuthost-bypass.rule=Host(\`${domain}\`) && (PathPrefix(\`/download\`) || PathPrefix(\`/api/m2m\`) || PathRegexp(\`/manifest..*.json\`) || PathRegexp(\`/favicon..*.svg\`))"
- "traefik.http.routers.shuthost-bypass.priority=100"
# Remove auth middleware for bypass routes`}
                        />

                        <p class="text-xs mt-2">
                            <em>
                                Replace backend references with your actual
                                configuration values. After configuring your
                                proxy rules, set{' '}
                                <code>
                                    {
                                        'auth = { type = "external", exceptions_version = 1 }'
                                    }
                                </code>{' '}
                                in the coordinator config to acknowledge the
                                exceptions. If this doesn't help, please raise
                                an issue.
                            </em>
                        </p>
                    </div>
                </section>
            </details>
        </section>
    );
}) satisfies AnyComponent;
