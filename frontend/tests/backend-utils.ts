import { spawnSync } from 'node:child_process';
import os from 'node:os';
import https, { Server } from 'node:https';
import fs from 'node:fs';
import path from 'node:path';
import { createPublicKey } from 'node:crypto';

// --- configuration --------------------------------------------------------
// canonical list of known configuration keys; kept in a fixed order so
// ports assigned by `assignedPortForConfig` remain deterministic.
export const CONFIG_KEYS = [
    'hosts-only',
    'hosts-and-clients',
    'nada',
    'auth-none',
    'auth-token',
    'auth-oidc',
    'auth-outdated-exceptions',
    'no-db',
] as const;

// helper that builds the config file path for a given key.  demo mode does
// not have a configuration file, so callers should special-case it.
export const configPathForKey = (key: typeof CONFIG_KEYS[number]) => `./tests/configs/${key}.toml`;

export const BACKEND_PATH = process.env['COVERAGE'] ? '../target/debug/shuthost_coordinator' : '../target/release/shuthost_coordinator';

// canonical list of all backend keys including the special demo entry.  Using
// a single array ensures loops in setup/teardown stay in sync and provides a
// convenient typed union.
// comprehensive list including the special `demo` entry.  having a
// separate array makes it easy to iterate through all possible backends
// during setup/teardown.
export const ALL_CONFIG_KEYS = [
    ...CONFIG_KEYS,
    'demo',
] as const;

export type ConfigKey = typeof ALL_CONFIG_KEYS[number];

export const BASE_PORT = 8081;
export const OIDC_PORT = BASE_PORT;

// Mock OIDC server host/port and base URL (DRY these values).  The port is
// coordinated with the auth-oidc backend so parallel workers pick unique
// ports; see `assignedOidcPort` in backend-utils.ts.
const OIDC_HOST = '127.0.0.1';
export const OIDC_BASE_URL = `https://${OIDC_HOST}:${OIDC_PORT}`;

/**
 * Kill any coordinator process listening for the given configuration key.
 *
 * This finds the deterministic port for `key`, enumerates PIDs listening on
 * that port, validates that each PID looks like our coordinator binary and
 * attempts a graceful shutdown.
 */
export const killTestBackendProcess = async (key: ConfigKey) => {
    const port = assignedPortForConfig(key);
    const pids = getPidsListeningOnPort(port);
    if (pids.length === 0) {
        console.log(`no processes found for config ${key} on port ${port}`);
        return;
    }
    for (const pid of pids) {
        const isExpected = validatePidIsExpected(pid, BACKEND_PATH);
        if (isExpected) {
            console.log(`terminating coordinator pid ${pid} for config ${key} on port ${port}`);
            await killPidGracefully(pid);
        } else {
            console.warn(`leaving pid ${pid} for config ${key} on port ${port} (not coordinator)`);
        }
    }
};

/**
 * Return the deterministic port number used by a given coordinator configuration.
 *
 * - known configs are assigned consecutive ports starting at BASE_PORT
 * - *demo mode* (configPath undefined) always returns DEMO_PORT, ensuring there
 *   is only a single demo backend regardless of the worker index
 * - any other value falls back to a per-worker port (for compatibility with
 *   manual usages but should not happen in normal tests)
 */
export const assignedPortForConfig = (configKey: ConfigKey) => {
    const idx = ALL_CONFIG_KEYS.indexOf(configKey);
    if (idx !== -1) {
        return BASE_PORT + 1 + idx;
    }
    throw new Error(`Unknown config key: ${configKey}`);
};

// ---------------------------------------------------------------------------
// port / process helpers
// ---------------------------------------------------------------------------

/**
 * Return an array of PIDs listening on the given TCP port. Cross-platform.
 *
 * On Unix we try to use `lsof`; on Windows we fall back to `netstat`.
 * If the lookup fails we return an empty array (caller will treat the port as
 * free).
 */
export const getPidsListeningOnPort = (port: number) => {
    try {
        if (os.platform() === 'win32') {
            const out = spawnSync('netstat', ['-ano'], { encoding: 'utf8' }).stdout;
            return Array.from(new Set(
                out
                    .split('\n')
                    .filter((l) => l.includes(`:${port}`))
                    .map((l) => l.trim().split(/\s+/).pop())
                    .filter(Boolean)
                    .map((s) => Number(s))
                    .filter((n) => !Number.isNaN(n))
            ));
        } else {
            const out = spawnSync('lsof', ['-nP', '-iTCP:' + port, '-sTCP:LISTEN', '-t'], { encoding: 'utf8' }).stdout;
            return out
                .split(/\s+/)
                .filter(Boolean)
                .map((s) => Number(s))
                .filter((n) => !Number.isNaN(n));
        }
    } catch {
        return [];
    }
};

/**
 * Return the full command line for a PID, or null if it can't be determined.
 */
const pidCommandLine = (pid: number) => {
    try {
        if (os.platform() === 'win32') {
            const out = spawnSync('powershell', ['-NoProfile', '-Command', `Get-CimInstance Win32_Process -Filter "ProcessId=${pid}" | Select-Object -ExpandProperty CommandLine`], { encoding: 'utf8' }).stdout.trim();
            return out || null;
        } else {
            const out = spawnSync('ps', ['-p', String(pid), '-o', 'command='], { encoding: 'utf8' }).stdout.trim();
            return out || null;
        }
    } catch {
        return null;
    }
};

/**
 * Return true if the given PID's command line contains the expected substring.
 * Used to make sure we only kill processes that look like our coordinator
 * binary.
 */
const validatePidIsExpected = (pid: number, expectedCmdSubstr: string) => {
    const cmd = pidCommandLine(pid);
    if (!cmd) return false;
    return cmd.includes(expectedCmdSubstr);
};

/**
 * Kill a process gracefully via SIGTERM, then SIGKILL if necessary. Returns
 * true if the process is no longer running after the call.
 */
const killPidGracefully = async (pid: number, timeoutMs = 5000) => {
    try {
        process.kill(pid, 'SIGTERM');
    } catch {
        // maybe already gone
    }
    const start = Date.now();
    return new Promise<boolean>((resolve) => {
        const check = () => {
            try {
                process.kill(pid, 0);
                if (Date.now() - start >= timeoutMs) {
                    try {
                        process.kill(pid, 'SIGKILL');
                    } catch { }
                    try {
                        process.kill(pid, 0);
                        resolve(false);
                    } catch {
                        resolve(true);
                    }
                    return;
                }
                setTimeout(check, 100);
            } catch {
                // process no longer exists
                resolve(true);
            }
        };
        check();
    });
};

export const startOidcMockServer = async () => {
    // Reuse the static TLS certificate/key that the backend already uses
    // during tests.  This removes the runtime dependency on openssl and
    // avoids creating temporary files.
    const thisDir = path.dirname(new URL(import.meta.url).pathname);
    const certPath = path.resolve(thisDir, 'configs', 'tls_cert.pem');
    const keyPath = path.resolve(thisDir, 'configs', 'tls_key.pem');

    const cert = fs.readFileSync(certPath, 'utf8');
    const key = fs.readFileSync(keyPath, 'utf8');

    // derive a JWK from the public key embedded in the cert
    const jwk = createPublicKey(cert).export({ format: 'jwk' });
    const jwks = {
        keys: [
            {
                ...jwk,
                use: 'sig',
                kid: 'test-key',
            },
        ],
    };

    const discovery = {
        issuer: OIDC_BASE_URL,
        authorization_endpoint: `${OIDC_BASE_URL}/authorize`,
        token_endpoint: `${OIDC_BASE_URL}/token`,
        jwks_uri: `${OIDC_BASE_URL}/jwks.json`,
        response_types_supported: ['code', 'id_token', 'token id_token'],
        grant_types_supported: ['authorization_code', 'refresh_token'],
        subject_types_supported: ['public'],
        id_token_signing_alg_values_supported: ['RS256'],
    };

    const serverOptions = { key, cert };

    let oidcServer = https.createServer(serverOptions, (req, res) => {
        if (req.url === '/.well-known/openid-configuration') {
            res.writeHead(200, { 'Content-Type': 'application/json' });
            res.end(JSON.stringify(discovery));
        } else if (req.url === '/jwks.json') {
            res.writeHead(200, { 'Content-Type': 'application/json' });
            res.end(JSON.stringify(jwks));
        } else {
            res.writeHead(404);
            res.end();
        }
    });
    return await new Promise<Server>((resolve, reject) => {
        // Bind explicitly to IPv4 loopback to avoid IPv6/IPv4 dual-stack issues.
        oidcServer!.listen(OIDC_PORT, OIDC_HOST, () => {
            console.log(`OIDC mock server running at ${OIDC_BASE_URL}`);
            resolve(oidcServer);
        });
        oidcServer!.on('error', (err) => reject(err));
    });
};
