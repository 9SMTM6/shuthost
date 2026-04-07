/// <reference types="node" />

import { defineConfig, devices } from '@playwright/test';
import { baseConfig } from './playwright.config';

const DEMO_URL = process.env['DEMO_URL'] ?? 'https://9smtm6.github.io/shuthost';

export default defineConfig({
    expect: baseConfig.expect,
    reporter: baseConfig.reporter,
    timeout: baseConfig.timeout,
    testDir: baseConfig.testDir,
    fullyParallel: true,
    testMatch: ['live-demo-functional.spec.ts', 'live-demo-visual.spec.ts'],
    outputDir: '../target/playwright-live-demo-results/',
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
