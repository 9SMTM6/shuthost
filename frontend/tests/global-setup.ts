import { execSync } from 'node:child_process';

export default async function globalSetup() {
    if (process.env.SKIP_BUILD) {
        console.log('SKIP_BUILD set — skipping coordinator build');
        return;
    }

    const flags = "--release --no-default-features"

    console.log(`Global setup: building coordinator (${flags})`);
    execSync(`cargo build ${flags}`, { cwd: '..', stdio: 'inherit' });
}
