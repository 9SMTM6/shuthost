import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
    globalSetup: './global-setup',
    testDir: './tests',
    timeout: 30000,
    expect: {
        timeout: 5000,
    },
    fullyParallel: true,
    // 'github' for GitHub Actions CI to generate annotations
    // default 'list' when running locally
    // HTML report to have easy access to the traces
    reporter: [[process.env.CI ? 'github' : 'list'], ['html']],
    use: {
        // Compute a per-worker baseURL so multiple workers can run parallel backends.
        baseURL: `http://127.0.0.1:${8081 + Number(process.env.TEST_PARALLEL_INDEX ?? process.env.TEST_WORKER_INDEX ?? '0')}`,
        trace: 'on',
        ignoreHTTPSErrors: true,
        // Explicitly use Playwright's Chromium browser so projects don't try to use a system Chrome
        browserName: 'chromium',
        channel: 'chromium',
    },
    projects: [
        {
            name: 'Desktop Light',
            testIgnore: ["mobile-navigation.spec.*"],
            use: { ...devices['Desktop Chrome'], colorScheme: 'light' },
        },
        {
            name: 'Desktop Dark',
            testIgnore: ["aria-snapshots.spec.*", "mobile-navigation.spec.*"],
            use: { ...devices['Desktop Chrome'], colorScheme: 'dark' },
        },
        {
            name: 'Mobile Light',
            testIgnore: ["aria-snapshots.spec.*"],
            use: { ...devices['Pixel 5'], colorScheme: 'light' },
        },
        {
            name: 'Mobile Dark',
            testIgnore: ["aria-snapshots.spec.*"],
            use: { ...devices['Pixel 5'], colorScheme: 'dark' },
        },
    ],
});
