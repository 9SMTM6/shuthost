import { spawnSync } from 'node:child_process';
import fs from 'fs';
import os from 'os';

// --- configuration --------------------------------------------------------
export const configs = {
    "hosts-only": './tests/configs/hosts-only.toml',
    "hosts-and-clients": './tests/configs/hosts-and-clients.toml',
    "nada": './tests/configs/nada.toml',
    "auth-none": './tests/configs/auth-none.toml',
    "auth-token": './tests/configs/auth-token.toml',
    "auth-oidc": './tests/configs/auth-oidc.toml',
    "auth-outdated-exceptions": './tests/configs/auth-outdated-exceptions.toml',
    "no-db": './tests/configs/no-db.toml',
};

// canonical list of all backend keys including the special demo entry.  Using
// a single array ensures loops in setup/teardown stay in sync and provides a
// convenient typed union.
export const ALL_CONFIG_KEYS = ([
    ...Object.keys(configs),
    'demo',
] as Array<"demo" | keyof typeof configs>);

export type ConfigKey = typeof ALL_CONFIG_KEYS[number];

export const BASE_PORT = 8081;

// demo mode always uses the port immediately following all named configs.
export const DEMO_PORT = BASE_PORT + Object.values(configs).length;

/**
 * Return the deterministic port number used by a given coordinator configuration.
 *
 * - known configs are assigned consecutive ports starting at BASE_PORT
 * - *demo mode* (configPath undefined) always returns DEMO_PORT, ensuring there
 *   is only a single demo backend regardless of the worker index
 * - any other value falls back to a per-worker port (for compatibility with
 *   manual usages but should not happen in normal tests)
 */
export function assignedPortForConfig(configKey: ConfigKey): number {
    // demo mode uses the dedicated port
    if (configKey === 'demo') {
        return DEMO_PORT;
    }

    // normal entries from `configs` (caller must supply one of the known keys or 'demo')
    const keys = Object.keys(configs);
    const idx = keys.indexOf(configKey);
    if (idx !== -1) {
        return BASE_PORT + idx;
    }

    // fallback per-worker port
    const parallelIndex = Number(
        process.env['TEST_PARALLEL_INDEX'] ||
        process.env['TEST_WORKER_INDEX'] ||
        '0'
    );
    return BASE_PORT + parallelIndex;
}

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
export function getPidsListeningOnPort(port: number): number[] {
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
}

/**
 * Return the full command line for a PID, or null if it can't be determined.
 */
export function pidCommandLine(pid: number): string | null {
    try {
        if (os.platform() === 'linux') {
            const content = fs.readFileSync(`/proc/${pid}/cmdline`, { encoding: 'utf8' });
            return content.replace(/\0/g, ' ').trim();
        } else {
            const out = spawnSync('ps', ['-p', String(pid), '-o', 'command='], { encoding: 'utf8' }).stdout.trim();
            return out || null;
        }
    } catch {
        return null;
    }
}

/**
 * Return true if the given PID's command line contains the expected substring.
 * Used to make sure we only kill processes that look like our coordinator
 * binary.
 */
export function validatePidIsExpected(pid: number, expectedCmdSubstr: string): boolean {
    const cmd = pidCommandLine(pid);
    if (!cmd) return false;
    return cmd.includes(expectedCmdSubstr);
}

/**
 * Kill a process gracefully via SIGTERM, then SIGKILL if necessary. Returns
 * true if the process is no longer running after the call.
 */
export async function killPidGracefully(pid: number, timeoutMs = 5000): Promise<boolean> {
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
}
