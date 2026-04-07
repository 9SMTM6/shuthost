import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
    globalSetup: './global-setup.ts',
    globalTeardown: './global-teardown.ts',
    testDir: './specs',
    outputDir: '../../target/playwright-oidc-test-results/',
    timeout: 60000,
    expect: {
        timeout: 10000,
    },
    fullyParallel: false,
    reporter: [[process.env['CI'] ? 'github' : 'list'], ['html']],
    use: {
        baseURL: 'https://127.0.0.1:18080',
        trace: 'on',
        ignoreHTTPSErrors: true,
        browserName: 'chromium',
        channel: 'chromium',
    },
    projects: [
        {
            name: 'Desktop Chrome',
            use: { ...devices['Desktop Chrome HiDPI'] },
        },
    ],
});
