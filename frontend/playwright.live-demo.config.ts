/// <reference types="node" />

import { defineConfig, devices } from '@playwright/test';

const DEMO_URL = process.env['DEMO_URL'] ?? 'https://9smtm6.github.io/shuthost';

export default defineConfig({
    testDir: './tests',
    testMatch: ['live-demo-functional.spec.ts', 'live-demo-visual.spec.ts'],
    outputDir: '../target/playwright-live-demo-results/',
    timeout: 30000,
    expect: {
        timeout: 5000,
        toHaveScreenshot: {
            maxDiffPixelRatio: process.env['PIXELPEEP'] === '1' ? 0 : 0.1,
            scale: 'device',
            pathTemplate:
                '{snapshotDir}/{testFileName}-snapshots/{arg}-{projectName}{ext}',
        },
    },
    fullyParallel: true,
    reporter: [[process.env['CI'] ? 'github' : 'list'], ['html']],
    use: {
        baseURL: DEMO_URL,
        trace: 'on',
        browserName: 'chromium',
        channel: 'chromium',
    },
    projects: [
        {
            name: 'Desktop Dark',
            use: { ...devices['Desktop Chrome HiDPI'], colorScheme: 'dark' },
        },
    ],
});
