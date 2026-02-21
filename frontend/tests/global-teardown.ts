import { ALL_CONFIG_KEYS, killTestBackendProcess, cleanupOidcMockServer } from './backend-utils';

const globalTeardown = async () => {
    console.log('Playwright global teardown: stopping backend processes');

    const tasks: Promise<void>[] = [];
    for (const key of ALL_CONFIG_KEYS) {
        tasks.push(killTestBackendProcess(key));
    }

    tasks.push(cleanupOidcMockServer().then(() => undefined));

    // no special demo logic required – ALL_CONFIG_KEYS includes it

    await Promise.all(tasks);
};

export default globalTeardown;
