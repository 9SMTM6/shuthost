import { spawn } from 'child_process';
import { configs, ALL_CONFIG_KEYS, ConfigKey, assignedPortForConfig, getPidsListeningOnPort, validatePidIsExpected, killPidGracefully } from './tests/backend-utils';
import { startOidcMockServer } from './tests/test-utils';
import net from 'net';

// Create a small helper that resolves when a port is free (or errors after timeout)
const waitForPortFree = (port: number, timeoutMs = 10000): Promise<void> => {
    return new Promise((resolve, reject) => {
        const start = Date.now();
        const tryBind = () => {
            const srv = net.createServer().once('error', (err: any) => {
                srv.close();
                if (err.code === 'EADDRINUSE') {
                    if (Date.now() - start > timeoutMs) {
                        reject(new Error(`port ${port} still in use after ${timeoutMs}ms`));
                    } else {
                        setTimeout(tryBind, 100);
                    }
                } else {
                    reject(err);
                }
            }).once('listening', () => {
                srv.close();
                resolve();
            }).listen(port);
        };
        tryBind();
    });
};

const globalSetup = async () => {
    console.log('Playwright global setup: starting backend processes');
    const backendBin = process.env['COVERAGE'] ? '../target/debug/shuthost_coordinator' : '../target/release/shuthost_coordinator';

    const startOne = async (key: ConfigKey, configPath?: string) => {
        const port = assignedPortForConfig(key);
        const pids = getPidsListeningOnPort(port);
        for (const pid of pids) {
            if (validatePidIsExpected(pid, backendBin)) {
                console.log(`killing stale coordinator pid ${pid} on port ${port}`);
                await killPidGracefully(pid);
            } else {
                throw new Error(`port ${port} is already in use by pid ${pid} which does not appear to be the coordinator`);
            }
        }
        await waitForPortFree(port, 5000);

        const args = configPath
            ? ['control-service', '--port', String(port), `--config=${configPath}`]
            : ['demo-service', '--port', String(port)];
        console.log(`spawning ${configPath ?? 'demo coordinator'} on port ${port}`);
        spawn(backendBin, args, { stdio: 'inherit', env: { RUST_LOG: 'error', ...process.env } });

        await new Promise<void>((resolve, reject) => {
            const deadline = Date.now() + 10000;
            const check = () => {
                if (getPidsListeningOnPort(port).length > 0) return resolve();
                if (Date.now() > deadline) return reject(new Error(`backend failed to bind port ${port}`));
                setTimeout(check, 100);
            };
            check();
        });
    };

    const tasks: Promise<void>[] = [];
    // start mock OIDC server if any config needs it and we're not skipping
    if (process.env['SKIP_OIDC'] !== '1') {
        console.log('global-setup: starting mock OIDC server');
        tasks.push(startOidcMockServer().then(() => undefined));
    }

    for (const key of ALL_CONFIG_KEYS) {
        if (key === 'demo') {
            tasks.push(startOne('demo'));
            continue;
        }
        if (process.env['SKIP_OIDC'] === '1' && key === 'auth-oidc') {
            console.log('global-setup: skipping OIDC config');
            continue;
        }
        // key is guaranteed to be one of configs at this point; cast for index
        tasks.push(startOne(key, configs[key]));
    }

    await Promise.all(tasks);
    console.log('all backends started');
};

export default globalSetup;
