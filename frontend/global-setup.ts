import { spawn } from 'child_process';
import { configs, assignedPortForConfig, getPidsListeningOnPort, validatePidIsExpected, killPidGracefully } from './tests/backend-utils';
import net from 'net';

// Create a small helper that resolves when a port is free (or errors after timeout)
function waitForPortFree(port: number, timeoutMs = 10000): Promise<void> {
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
}

export default async function globalSetup() {
    console.log('Playwright global setup: starting backend processes');
    const backendBin = process.env['COVERAGE'] ? '../target/debug/shuthost_coordinator' : '../target/release/shuthost_coordinator';

    // start all the named configs first
    for (const configPath of Object.values(configs)) {
        if (process.env['SKIP_OIDC'] === '1' && configPath.includes('auth-oidc')) {
            console.log('global-setup: skipping OIDC config');
            continue;
        }
        const port = assignedPortForConfig(configPath);

        // if something is already listening, try to kill it if it's our binary
        const pids = getPidsListeningOnPort(port);
        for (const pid of pids) {
            if (validatePidIsExpected(pid, backendBin)) {
                console.log(`killing stale coordinator pid ${pid} on port ${port}`);
                killPidGracefully(pid);
            } else {
                throw new Error(
                    `port ${port} is already in use by pid ${pid} which does not appear to be the coordinator`);
            }
        }

        await waitForPortFree(port, 5000);

        const args = ['control-service', '--port', String(port), `--config=${configPath}`];
        console.log(`spawning coordinator for ${configPath} on port ${port}`);
        spawn(backendBin, args, { stdio: 'inherit', env: process.env });
        // don't await proc; we just need to wait for the HTTP listener
        await new Promise<void>((resolve, reject) => {
            const deadline = Date.now() + 10000;
            const check = () => {
                const pids2 = getPidsListeningOnPort(port);
                if (pids2.length > 0) return resolve();
                if (Date.now() > deadline) return reject(new Error(`backend failed to bind port ${port}`));
                setTimeout(check, 100);
            };
            check();
        });
    }

    // finally spawn a demo-service on the default/worker port (undefined config)
    const demoPort = assignedPortForConfig(undefined);
    // ensure we didn't already spawn one above (parallelIndex might collide with config)
    if (!Object.values(configs).some((p) => assignedPortForConfig(p) === demoPort)) {
        console.log(`spawning demo coordinator on port ${demoPort}`);
        const pids = getPidsListeningOnPort(demoPort);
        for (const pid of pids) {
            if (validatePidIsExpected(pid, backendBin)) {
                console.log(`killing stale coordinator pid ${pid} on port ${demoPort}`);
                killPidGracefully(pid);
            } else {
                throw new Error(
                    `port ${demoPort} is already in use by pid ${pid} which does not appear to be the coordinator`);
            }
        }
        await waitForPortFree(demoPort, 5000);
        const args = ['demo-service', '--port', String(demoPort)];
        spawn(backendBin, args, { stdio: 'inherit', env: process.env });
        await new Promise<void>((resolve, reject) => {
            const deadline = Date.now() + 10000;
            const check = () => {
                if (getPidsListeningOnPort(demoPort).length > 0) return resolve();
                if (Date.now() > deadline) return reject(new Error(`demo backend failed to bind port ${demoPort}`));
                setTimeout(check, 100);
            };
            check();
        });
    }

    console.log('all backends started');
}
