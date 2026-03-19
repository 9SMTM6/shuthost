import { execSync } from 'child_process';
import path from 'path';
import { killCoordinator } from './global-setup';

const THIS_DIR = path.dirname(new URL(import.meta.url).pathname);
const DEX_COMPOSE_DIR = path.resolve(THIS_DIR, 'providers', 'dex');

const stopDex = () => {
    console.log('Stopping Dex provider...');
    try {
        execSync('docker compose down', { cwd: DEX_COMPOSE_DIR, stdio: 'inherit' });
    } catch (err) {
        console.warn(`docker compose down failed: ${String(err)}`);
    }
};

const globalTeardown = async () => {
    console.log('OIDC integration global teardown');
    await killCoordinator();
    stopDex();
    console.log('OIDC integration teardown complete');
};

export default globalTeardown;
