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

export const BASE_PORT = 8081;

/**
 * Return the deterministic port number used by a given coordinator configuration.
 * If `configPath` is not among the known configs or is undefined, falls back to
 * a per-worker port using the TEST_PARALLEL_INDEX/TEST_WORKER_INDEX env vars.
 */
export function assignedPortForConfig(configPath?: string): number {
  if (configPath) {
    const values = Object.values(configs);
    const idx = values.indexOf(configPath);
    if (idx !== -1) {
      return BASE_PORT + idx;
    }
  }

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
export function killPidGracefully(pid: number, timeoutMs = 5000): boolean {
  try {
    process.kill(pid, 'SIGTERM');
  } catch {
    // maybe already gone
  }
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    try {
      process.kill(pid, 0);
      // still alive; wait and retry
      sleepSync(100);
    } catch {
      return true; // exited
    }
  }
  try {
    process.kill(pid, 'SIGKILL');
  } catch {}
  try {
    process.kill(pid, 0);
    return false;
  } catch {
    return true;
  }
}

function sleepSync(ms: number) {
  const end = Date.now() + ms;
  while (Date.now() < end) {}
}
