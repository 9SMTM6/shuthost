import { execSync, spawn, spawnSync } from 'child_process';
import fs from 'fs';
import path from 'path';
import http from 'http';

export const COORD_PORT = 18080;
export const DEX_PORT = 5556;
export const DEX_ISSUER = `http://127.0.0.1:${DEX_PORT}`;
export const COORD_BASE_URL = `https://127.0.0.1:${COORD_PORT}`;

const THIS_DIR = path.dirname(new URL(import.meta.url).pathname);
const REPO_ROOT = path.resolve(THIS_DIR, '..', '..');
export const CERT_DIR = path.resolve(THIS_DIR, 'certs');
export const CERT_PATH = path.resolve(CERT_DIR, 'tls_cert.pem');
export const KEY_PATH = path.resolve(CERT_DIR, 'tls_key.pem');
const CONFIG_TMPL = path.resolve(THIS_DIR, 'configs', 'coordinator-dex.tmpl.toml');
const CONFIG_OUT = path.resolve(THIS_DIR, 'configs', 'coordinator-dex.toml');
const DEX_COMPOSE_DIR = path.resolve(THIS_DIR, 'providers', 'dex');
const BACKEND_PATH = process.env['COVERAGE']
    ? path.resolve(REPO_ROOT, 'target', 'debug', 'shuthost_coordinator')
    : path.resolve(REPO_ROOT, 'target', 'release', 'shuthost_coordinator');

const buildCoordinator = () => {
    if (process.env['SKIP_BUILD']) {
        console.log('SKIP_BUILD set — skipping coordinator build');
        return;
    }
    const flags = process.env['COVERAGE'] ? '' : '--release';
    console.log(`OIDC global-setup: building coordinator (${flags})`);
    const env = { ...process.env, OIDC_DANGER_ACCEPT_INVALID_CERTS: '1' };
    execSync(`cargo build ${flags} --bin shuthost_coordinator`, {
        cwd: REPO_ROOT,
        stdio: 'inherit',
        env,
    });
};

const generateCert = () => {
    if (fs.existsSync(CERT_PATH) && fs.existsSync(KEY_PATH)) {
        console.log('TLS cert/key already exist, skipping generation');
        return;
    }
    fs.mkdirSync(CERT_DIR, { recursive: true });
    console.log('Generating self-signed TLS cert/key via openssl...');
    execSync(
        `openssl req -x509 -newkey rsa:2048 -keyout "${KEY_PATH}" -out "${CERT_PATH}" \
         -days 3650 -nodes -subj "/CN=127.0.0.1" \
         -addext "subjectAltName=IP:127.0.0.1"`,
        { stdio: 'inherit' },
    );
    console.log(`Cert written to ${CERT_PATH}`);
    console.log(`Key written to ${KEY_PATH}`);
};

const startDex = () => {
    console.log('Starting Dex provider via docker compose...');
    execSync('docker compose up -d --wait', {
        cwd: DEX_COMPOSE_DIR,
        stdio: 'inherit',
    });
};

const waitForDexDiscovery = async () => {
    const url = `${DEX_ISSUER}/.well-known/openid-configuration`;
    console.log(`Waiting for Dex discovery endpoint: ${url}`);
    const deadline = Date.now() + 30000;
    await new Promise<void>((resolve, reject) => {
        const attempt = () => {
            http.get(url, (res) => {
                if (res.statusCode === 200) {
                    console.log('Dex is ready');
                    resolve();
                } else {
                    retry();
                }
                res.resume();
            }).on('error', () => retry());
        };
        const retry = () => {
            if (Date.now() > deadline) {
                reject(new Error(`Dex did not become ready at ${url} within 30s`));
            } else {
                setTimeout(attempt, 500);
            }
        };
        attempt();
    });
};

const writeCoordinatorConfig = () => {
    const tmpl = fs.readFileSync(CONFIG_TMPL, 'utf-8');
    const config = tmpl
        .replaceAll('{{COORD_PORT}}', String(COORD_PORT))
        .replaceAll('{{DEX_PORT}}', String(DEX_PORT))
        .replaceAll('{{CERT_PATH}}', CERT_PATH)
        .replaceAll('{{KEY_PATH}}', KEY_PATH);
    fs.writeFileSync(CONFIG_OUT, config, 'utf-8');
    console.log(`Coordinator config written to ${CONFIG_OUT}`);
};

export const getPidsListeningOnPort = (port: number): number[] => {
    try {
        const out = spawnSync('lsof', ['-nP', `-iTCP:${port}`, '-sTCP:LISTEN', '-t'], { encoding: 'utf8' });
        return (out.stdout ?? '')
            .split(/\s+/)
            .filter(Boolean)
            .map((s) => Number(s))
            .filter((n) => !Number.isNaN(n));
    } catch {
        return [];
    }
};

const killPid = async (pid: number) => {
    try { process.kill(pid, 'SIGTERM'); } catch { /* already gone */ }
    await new Promise<void>((resolve) => {
        const check = () => {
            try {
                process.kill(pid, 0);
                setTimeout(check, 100);
            } catch {
                resolve();
            }
        };
        setTimeout(check, 100);
    });
};

export const killCoordinator = async () => {
    const pids = getPidsListeningOnPort(COORD_PORT);
    for (const pid of pids) {
        console.log(`Killing coordinator pid ${pid}`);
        await killPid(pid);
    }
};

const startCoordinator = async () => {
    await killCoordinator();
    console.log(`Spawning coordinator on port ${COORD_PORT}`);
    spawn(
        BACKEND_PATH,
        ['control-service', '--port', String(COORD_PORT), `--config=${CONFIG_OUT}`],
        { stdio: 'inherit', env: { RUST_LOG: 'info', ...process.env } },
    );
    await new Promise<void>((resolve, reject) => {
        const deadline = Date.now() + 20000;
        const check = () => {
            if (getPidsListeningOnPort(COORD_PORT).length > 0) return resolve();
            if (Date.now() > deadline) return reject(new Error(`Coordinator failed to bind port ${COORD_PORT}`));
            setTimeout(check, 100);
        };
        check();
    });
    console.log(`Coordinator is up on port ${COORD_PORT}`);
};

const globalSetup = async () => {
    console.log('OIDC integration global setup');
    buildCoordinator();
    generateCert();
    startDex();
    await waitForDexDiscovery();
    writeCoordinatorConfig();
    await startCoordinator();
    console.log('OIDC integration setup complete');
};

export default globalSetup;
