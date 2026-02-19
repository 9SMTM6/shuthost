/// <reference lib="dom" />

import { Page } from '@playwright/test';
import { spawn, ChildProcess } from 'node:child_process';
import https from 'https';
import fs from 'fs';
import os from 'os';
import path from 'path';
import { execSync } from 'child_process';
import { Server } from 'node:https';

// let staticServer: https.Server | undefined;
let staticTmpDir: string | undefined;

export const configs = {
  "hosts-only": './tests/configs/hosts-only.toml',
  "hosts-and-clients": './tests/configs/hosts-and-clients.toml',
  "nada": './tests/configs/nada.toml',
  "auth-none": './tests/configs/auth-none.toml',
  "auth-token": './tests/configs/auth-token.toml',
  "auth-oidc": './tests/configs/auth-oidc.toml',
  "auth-outdated-exceptions": './tests/configs/auth-outdated-exceptions.toml',
  "no-db": './tests/configs/no-db.toml',
}

// Get the test port for parallel workers to avoid conflicts.
export const getTestPort = (): number => {
  const parallelIndex = Number(process.env['TEST_PARALLEL_INDEX'] ?? process.env['TEST_WORKER_INDEX'] ?? '0');
  return 8081 + parallelIndex;
}

// Mock OIDC server host/port and base URL (DRY these values)
const OIDC_HOST = '127.0.0.1';
const OIDC_PORT = 8443;
export const OIDC_BASE_URL = `https://${OIDC_HOST}:${OIDC_PORT}`;

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
  // Propagate environment and enable runtime acceptance of invalid OIDC certs
  // when using the `auth-oidc` test config so the coordinator trusts the test
  // self-signed certificate without requiring a rebuild.
  const childEnv: any = { RUST_LOG: "error", ...process.env };
  if (configPath && configPath.includes('auth-oidc')) {
    childEnv['OIDC_DANGER_ACCEPT_INVALID_CERTS'] = '1';
  }
  const proc = spawn(
    backendBin,
    args,
    { stdio: 'inherit', env: childEnv }
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
    // Recursively sanitize all text nodes in the DOM
    const fullUrlRegexes: RegExp[] = [
      /https?:\/\/127\.0\.0\.1:\d+/g,
      /https?:\/\/localhost:\d+/g,
      /http:\/\/127\.0\.0\.1:\d+/g,
      /http:\/\/localhost:\d+/g
    ];
    const domainRegexes: RegExp[] = [
      /127\.0\.0\.1:\d+/g,
      /localhost:\d+/g,
      /127\.0\.0\.1/g,
      /localhost/g
    ];
    /** Replace environment-dependent URLs and domains in a string.*/
    const sanitizeText = (text: string): string => {
      let sanitized = text;
      fullUrlRegexes.forEach((r: RegExp) => {
        sanitized = sanitized.replace(r, '<protocol://base_url>');
      });
      domainRegexes.forEach((r: RegExp) => {
        sanitized = sanitized.replace(r, '<base_url>');
      });
      return sanitized;
    };
    function isHTMLElement(node: Node): node is HTMLElement {
      return node.nodeType === Node.ELEMENT_NODE;
    }

    /** Recursively walk the DOM and sanitize all text nodes. */
    const walk = (node: Node): void => {
      if (node.nodeType === Node.TEXT_NODE) {
        node.textContent = sanitizeText(node.textContent || '');
      } else if (isHTMLElement(node)) {
        // Special handling for config location
        if (node.id === 'config-location') {
          node.textContent = '<coordinator_config_location>';
        }
        node.childNodes.forEach(walk);
      }
    };
    walk(document.body);
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

export async function startStaticServer() {
  // Create a temporary directory for the generated cert/key
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'mock-oidc-'));
  staticTmpDir = tmpDir;

  const keyPath = path.join(tmpDir, 'key.pem');
  const certPath = path.join(tmpDir, 'cert.pem');
  const pubPath = path.join(tmpDir, 'pub.pem');

  // Generate key + self-signed cert using openssl (simple form). This is
  // the earlier, more permissive invocation that OpenSSL generated for us
  // previously and may be accepted by the test coordinator setup.
  execSync(`openssl req -x509 -newkey rsa:2048 -nodes -keyout ${keyPath} -out ${certPath} -days 1 -subj "/CN=127.0.0.1"`);
  // Extract public key in PEM form
  execSync(`openssl rsa -in ${keyPath} -pubout -out ${pubPath}`);

  const key = fs.readFileSync(keyPath, 'utf8');
  const cert = fs.readFileSync(certPath, 'utf8');
  const pubPem = fs.readFileSync(pubPath, 'utf8');

  const publicKeyBase64 = pubPem.replace(/-----BEGIN PUBLIC KEY-----|-----END PUBLIC KEY-----|\n/g, '');

  const jwks = {
    keys: [
      {
        kty: 'RSA',
        use: 'sig',
        kid: 'test-key',
        n: publicKeyBase64,
        e: 'AQAB',
      },
    ],
  };

  const discovery = {
    issuer: OIDC_BASE_URL,
    authorization_endpoint: `${OIDC_BASE_URL}/authorize`,
    token_endpoint: `${OIDC_BASE_URL}/token`,
    jwks_uri: `${OIDC_BASE_URL}/jwks.json`,
  };

  const serverOptions = { key, cert };

  let staticServer = https.createServer(serverOptions, (req, res) => {
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
    staticServer!.listen(OIDC_PORT, OIDC_HOST, () => {
      console.log(`Static Mock OIDC server running at ${OIDC_BASE_URL}`);
      resolve(staticServer);
    });
    staticServer!.on('error', (err) => reject(err));
  });
}

export async function stopStaticServer(_param: any) {
  // disabled check during testing
    // if (staticServer) {
        // staticServer!.close(() => {
        //     console.log('Static Mock OIDC server stopped.');
        // });
        // staticServer = undefined;
    // }
  if (staticTmpDir) {
    try {
      fs.rmSync(staticTmpDir, { recursive: true, force: true });
    } catch (e) {
      // ignore cleanup errors
    }
    staticTmpDir = undefined;
  }
}
