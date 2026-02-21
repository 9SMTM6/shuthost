import { spawn } from 'child_process';
import {
    ALL_CONFIG_KEYS,
    ConfigKey,
    configPathForKey,
    killTestBackendProcess,
    assignedPortForConfig,
    getPidsListeningOnPort,
    BACKEND_PATH,
    startOidcMockServer,
} from './backend-utils';

const globalSetup = async () => {
    console.log('Playwright global setup: starting backend processes');
    const backendBin = BACKEND_PATH;

    // start mock OIDC server for any config that needs it
    console.log('global-setup: starting mock OIDC server');
    await startOidcMockServer().then(() => undefined);

    const startOne = async (key: ConfigKey) => {
        const port = assignedPortForConfig(key);
        await killTestBackendProcess(key);

        const args = key === 'demo'
            ? ['demo-service', '--port', String(port)]
            : ['control-service', '--port', String(port), `--config=${configPathForKey(key)}`];
        console.log(`spawning ${key} coordinator on port ${port}`);
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

    for (const key of ALL_CONFIG_KEYS) {
        if (key === 'demo') {
            tasks.push(startOne('demo'));
            continue;
        }
        tasks.push(startOne(key));
    }

    await Promise.all(tasks);
    console.log('all backends started');
};

export default globalSetup;
