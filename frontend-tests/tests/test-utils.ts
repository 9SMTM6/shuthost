export const configs = {
  "hosts-only": './configs/hosts-only.toml',
  "hosts-and-clients": './configs/hosts-and-clients.toml',
  "nada": './configs/nada.toml',
  "no-auth": './configs/no-auth.toml',
}

export const screenshotOpts = { animations: 'disabled', maxDiffPixelRatio: 0.03 } as const;

// Utilities to build, start, wait for, and stop the Rust backend used by Playwright tests.
export async function waitForServerReady(host: string, port: number, timeout = 30000) {
  const start = Date.now();
  const https = await import('node:https');
  while (Date.now() - start < timeout) {
    try {
      await new Promise<void>((resolve, reject) => {
        const req = https.request({ hostname: host, port, path: '/', method: 'GET', rejectUnauthorized: false }, (res) => {
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
  throw new Error(`Timed out waiting for server at ${host}:${port}`);
}

export async function startBackend(configPath: string) {
  // Build release binary and spawn the control-service with provided config.
  const { execSync, spawn } = await import('node:child_process');
  // repo layout: frontend-tests is cwd when Playwright runs tests, binary is at ../target/release
  execSync('cargo build --release --no-default-features', { cwd: '..', stdio: 'inherit' });
  const backendBin = '../target/release/shuthost_coordinator';
  const proc = spawn(backendBin, ['control-service', '--config', configPath], { stdio: 'inherit' });
  await waitForServerReady('127.0.0.1', 8081, 30000);
  return proc;
}

export function stopBackend(proc: any) {
  try {
    if (proc) proc.kill();
  } catch (e) {
    // ignore
  }
}
