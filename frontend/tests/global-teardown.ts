import { ALL_CONFIG_KEYS, ConfigKey, assignedPortForConfig, getPidsListeningOnPort, validatePidIsExpected, killPidGracefully } from './backend-utils';
import { stopOidcMockServer } from './test-utils';
import { getBackendPath } from './global-setup';

const globalTeardown = async () => {
    console.log('Playwright global teardown: stopping backend processes');
    const backendBin = getBackendPath();

    const killOne = async (key: ConfigKey) => {
        const port = assignedPortForConfig(key);
        const pids = getPidsListeningOnPort(port);
        for (const pid of pids) {
            if (validatePidIsExpected(pid, backendBin)) {
                console.log(`terminating coordinator pid ${pid} on port ${port}`);
                await killPidGracefully(pid);
            } else {
                console.warn(`leaving pid ${pid} on port ${port} (not coordinator)`);
            }
        }
    };

    const tasks: Promise<void>[] = [];
    for (const key of ALL_CONFIG_KEYS) {
        tasks.push(killOne(key));
    }

    // stop mock OIDC server if we started it
    if (process.env['SKIP_OIDC'] !== '1') {
        console.log('global-teardown: stopping mock OIDC server');
        tasks.push(stopOidcMockServer(undefined).then(() => undefined));
    }

    // no special demo logic required – ALL_CONFIG_KEYS includes it

    await Promise.all(tasks);
};

export default globalTeardown;
