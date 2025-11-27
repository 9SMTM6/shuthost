import { Page } from '@playwright/test';
import { spawn, ChildProcess } from 'node:child_process';

export const configs = {
  "hosts-only": './tests/configs/hosts-only.toml',
  "hosts-and-clients": './tests/configs/hosts-and-clients.toml',
  "nada": './tests/configs/nada.toml',
  "auth-none": './tests/configs/auth-none.toml',
  "auth-token": './tests/configs/auth-token.toml',
  "auth-oidc": './tests/configs/auth-oidc.toml',
  "auth-outdated-exceptions": './tests/configs/auth-outdated-exceptions.toml',
}

// Get the test port for parallel workers to avoid conflicts.
export const getTestPort = (): number => {
  const parallelIndex = Number(process.env['TEST_PARALLEL_INDEX'] ?? process.env['TEST_WORKER_INDEX'] ?? '0');
  return 8081 + parallelIndex;
}

// Utilities to build, start, wait for, and stop the Rust backend used by Playwright tests.
export const waitForServerReady = async (port: number, useTls = false, timeout = 30000) => {
  const start = Date.now();
  const protocol = useTls ? await import('node:https') : await import('node:http');
  while (Date.now() - start < timeout) {
    try {
      await new Promise<void>((resolve, reject) => {
        const req = protocol.request({
          hostname: "127.0.0.1",
          port,
          path: '/',
          method: 'GET',
          rejectUnauthorized: false // Allow self-signed certificates in tests
        }, (res) => {
          res.resume();
          resolve();
        });
        req.on('error', reject);
        req.end();
      });
      return;
    } catch (e) {
      await new Promise((r) => setTimeout(r, 250));
    }
  }
  throw new Error(`Timed out waiting for server at 127.0.0.1:${port}`);
}

export const startBackend = async (configPath?: string, useTls = false, command = 'control-service') => {
  // Spawn the backend with provided config (if any). Build is performed in globalSetup.
  const backendBin = process.env['COVERAGE'] ? '../target/debug/shuthost_coordinator' : '../target/release/shuthost_coordinator';
  // Determine per-worker port to allow parallel test workers.
  // fall back to 0 for single-worker runs.
  const port = getTestPort();
  const args = [command, '--port', String(port)];
  if (configPath) {
    args.push(`--config=${configPath}`);
  }
  const proc = spawn(
    backendBin,
    args,
    { stdio: 'inherit', env: { RUST_LOG: "error", ...process.env } }
  );
  await waitForServerReady(port, useTls, 30000);
  return proc;
}

export const stopBackend = async (proc?: ChildProcess) => {
  if (!proc) return;

  // Try a graceful shutdown first
  try {
    // Send SIGTERM instead of SIGKILL
    proc.kill('SIGTERM');
  } catch (_) {
    return;
  }

  // Wait a bit for the process to exit and flush coverage data
  await new Promise<void>(resolve => {
    const timeout = setTimeout(() => {
      // If it’s still hanging, force kill it
      try { proc.kill('SIGKILL'); } catch { }
      resolve();
    }, 3000);

    proc.on('exit', () => {
      clearTimeout(timeout);
      resolve();
    });
  });
}

/** Replaces environment-dependent values like URLs and config paths with placeholders for generic snapshots */
export const sanitizeEnvironmentDependents = async (page: Page) => {
  await page.evaluate(() => {
    // Replace base URLs and domains in all text content
    const fullUrlRegex = /https?:\/\/127\.0\.0\.1:\d+/g;
    const domainRegex = /127\.0\.0\.1:\d+/g;
    [
      '#authelia-config',
      '#traefik-config',
      '#host-install-command',
      '#client-install-command-sh',
      '#client-install-command-ps1',
    ].forEach(selector => {
      const el = document.querySelector(selector);
      if (el?.textContent) {
        el.textContent = el.textContent.replaceAll(fullUrlRegex, '<protocol://base_url>').replaceAll(domainRegex, '<base_url>');
      }
    });
    // Replace config locations
    document.querySelectorAll('#config-location').forEach(el => { el.textContent = '<coordinator_config_location>'; });
  });
}

export const expand_and_sanitize_host_install = async (page: Page) => {
  await page.goto('#hosts');
  // Open the collapsible by checking the toggle input
  // The checkbox input is hidden (CSS); click the visible header/label instead.
  await page.waitForSelector('#host-install-header');
  await page.click('#host-install-header');
  await page.waitForSelector('#host-install-content', { state: 'visible' });
  // Sanitize dynamic install command and config path for stable snapshots
  await sanitizeEnvironmentDependents(page);
}
