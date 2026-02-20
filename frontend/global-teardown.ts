import { ALL_CONFIG_KEYS, ConfigKey, assignedPortForConfig, getPidsListeningOnPort, validatePidIsExpected, killPidGracefully } from './tests/backend-utils';

const globalTeardown = async () => {
    console.log('Playwright global teardown: stopping backend processes');
    const backendBin = process.env['COVERAGE'] ? '../target/debug/shuthost_coordinator' : '../target/release/shuthost_coordinator';

    const killOne = async (key: ConfigKey) => {
        const port = assignedPortForConfig(key);
        const pids = getPidsListeningOnPort(port);
        for (const pid of pids) {
            if (validatePidIsExpected(pid, backendBin)) {
                console.log(`terminating coordinator pid ${pid} on port ${port}`);
                await killPidGracefully(pid);
            } else {
                console.log(`leaving pid ${pid} on port ${port} (not coordinator)`);
            }
        }
    };

    const tasks: Promise<void>[] = [];
    for (const key of ALL_CONFIG_KEYS) {
        tasks.push(killOne(key));
    }

    // no special demo logic required – ALL_CONFIG_KEYS includes it

    await Promise.all(tasks);
};

export default globalTeardown;
