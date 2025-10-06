export const configs = {
  "hosts-only": './configs/hosts-only.toml',
  "hosts-and-clients": './configs/hosts-and-clients.toml',
  "nada": './configs/nada.toml',
  "no-auth": './configs/no-auth.toml',
}

// Utilities to build, start, wait for, and stop the Rust backend used by Playwright tests.
export async function waitForServerReady(port: number, timeout = 30000) {
  const start = Date.now();
  const http = await import('node:http');
  while (Date.now() - start < timeout) {
    try {
      await new Promise<void>((resolve, reject) => {
        const req = http.request({ hostname: "127.0.0.1", port, path: '/', method: 'GET' }, (res: any) => {
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

export async function startBackend(configPath: string) {
  // Spawn the control-service with provided config. Build is performed in globalSetup.
  const { spawn } = await import('node:child_process');
  const backendBin = '../target/release/shuthost_coordinator';
  // Determine per-worker port to allow parallel test workers.
  // fall back to 0 for single-worker runs.
  const parallelIndex = Number(process.env.TEST_PARALLEL_INDEX ?? process.env.TEST_WORKER_INDEX ?? '0');
  const port = 8081 + parallelIndex;
  const proc = spawn(
    backendBin,
    ['control-service', '--config', configPath, '--port', String(port)],
    { stdio: 'inherit' }
  );
  await waitForServerReady(port, 30000);
  return proc;
}

export function stopBackend(proc: any) {
  try {
    if (proc) proc.kill();
  } catch (e) {
    // ignore
  }
}
