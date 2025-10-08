import { Page } from '@playwright/test';

export const configs = {
  "hosts-only": './tests/configs/hosts-only.toml',
  "hosts-and-clients": './tests/configs/hosts-and-clients.toml',
  "nada": './tests/configs/nada.toml',
  "auth-none": './tests/configs/auth-none.toml',
  "auth-token": './tests/configs/auth-token.toml',
  "auth-oidc": './tests/configs/auth-oidc.toml',
}

// Utilities to build, start, wait for, and stop the Rust backend used by Playwright tests.
export async function waitForServerReady(port: number, useTls = false, timeout = 30000) {
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
        }, (res: any) => {
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

export async function startBackend(configPath: string, useTls = false) {
  // Spawn the control-service with provided config. Build is performed in globalSetup.
  const { spawn } = await import('node:child_process');
  const backendBin = '../target/release/shuthost_coordinator';
  // Determine per-worker port to allow parallel test workers.
  // fall back to 0 for single-worker runs.
  const parallelIndex = Number(process.env['TEST_PARALLEL_INDEX'] ?? process.env['TEST_WORKER_INDEX'] ?? '0');
  const port = 8081 + parallelIndex;
  const proc = spawn(
    backendBin,
    ['control-service', '--config', configPath, '--port', String(port)],
    { stdio: 'inherit' }
  );
  await waitForServerReady(port, useTls, 30000);
  return proc;
}

export function stopBackend(proc: any) {
  try {
    if (proc) proc.kill();
  } catch (e) {
    // ignore
  }
}

export const expand_and_sanitize_host_install = async (page: Page) => {
  await page.goto('#hosts');
  // Open the collapsible by checking the toggle input
  // The checkbox input is hidden (CSS); click the visible header/label instead.
  await page.waitForSelector('#host-install-header');
  await page.click('#host-install-header');
  await page.waitForSelector('#host-install-content', { state: 'visible' });
  // Sanitize dynamic install command and config path for stable snapshots
  await page.evaluate(() => {
    const cmd = document.querySelector('#host-install-command');
    if (cmd) cmd.textContent = '<<INSTALL_COMMAND_REDACTED>>';
    document.querySelectorAll('#config-location').forEach(el => { el.textContent = '<<COORDINATOR_CONFIG>>'; });
  });
}