import { test as setup } from '@playwright/test';
import { execSync } from 'node:child_process';

const buildCoordinator = () => {
    if (process.env['SKIP_BUILD']) {
        console.log('SKIP_BUILD set — skipping coordinator build');
        return;
    }

    const flags = process.env['COVERAGE'] ? "" : "--release"

    console.log(`Global setup: building coordinator (${flags})`);
    const env = { ...process.env, OIDC_DANGER_ACCEPT_INVALID_CERTS: '1' };
    execSync(`cargo build ${flags}`, { cwd: '..', stdio: 'inherit', env });
}

setup("Compile coordinator before tests", buildCoordinator)
