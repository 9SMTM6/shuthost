/// <reference types="node" />

import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
    globalSetup: './tests/global-setup',
    testDir: './tests',
    timeout: 30000,
    expect: {
        timeout: 5000,
        toHaveScreenshot: {
            maxDiffPixelRatio: 0.01,
            // have nice hi-res screenshots that can double as images in Docs.
            scale: 'device',
            // Strip the platform from the file name,
            // so that we don't need to execute on every platform after changes.
            // 
            // This might cause issues, if so we'll have to revert.
            pathTemplate: "{snapshotDir}/{testFileName}-snapshots/{arg}-{projectName}{ext}",
            // Default: 
            // pathTemplate: "{snapshotDir}/{testFileName}-snapshots/{arg}-{projectName}-{platform}{ext}",
        }
    },
    fullyParallel: true,
    // 'github' for GitHub Actions CI to generate annotations
    // default 'list' when running locally
    // HTML report to have easy access to the traces
    reporter: [[process.env['CI'] ? 'github' : 'list'], ['html']],
    use: {
        // Compute a per-worker baseURL so multiple workers can run parallel backends.
        baseURL: `http://127.0.0.1:${8081 + Number(process.env['TEST_PARALLEL_INDEX'] ?? process.env['TEST_WORKER_INDEX'] ?? '0')}`,
        trace: 'on',
        ignoreHTTPSErrors: true,
        // Explicitly use Playwright's Chromium browser so projects don't try to use a system Chrome
        browserName: 'chromium',
        channel: 'chromium',
    },
    projects: [
        {
            name: 'Desktop Dark',
            testIgnore: ["mobile-navigation.spec.*"],
            use: { ...devices['Desktop Chrome HiDPI'], colorScheme: 'dark' },
        },
        {
            name: 'Desktop Light',
            testIgnore: ["aria-snapshots.spec.*", "pwa-installability.spec.*", "mobile-navigation.spec.*"],
            use: { ...devices['Desktop Chrome HiDPI'], colorScheme: 'light' },
        },
        {
            name: 'Mobile Dark',
            testIgnore: ["aria-snapshots.spec.*", "pwa-installability.spec.*"],
            use: { ...devices['Pixel 7'], colorScheme: 'dark' },
        },
        {
            name: 'Mobile Light',
            testIgnore: ["aria-snapshots.spec.*", "pwa-installability.spec.*"],
            use: { ...devices['Pixel 7'], colorScheme: 'light' },
        },
    ],
});
