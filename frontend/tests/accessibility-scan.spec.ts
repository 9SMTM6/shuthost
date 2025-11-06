import { test, expect } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright'; // 1
import { startBackend, stopBackend, configs, expand_and_sanitize_host_install } from './test-utils';
import { ChildProcess } from 'node:child_process';

let backendProcess: ChildProcess | undefined;

test.beforeAll(async () => {
    backendProcess = await startBackend(configs["hosts-and-clients"]);
});

test.describe('main page(s)', () => {
    test('hosts page should not have any automatically detectable accessibility issues', async ({ page }) => {
        await expand_and_sanitize_host_install(page);

        const accessibilityScanResults = await new AxeBuilder({ page }).analyze();

        expect(accessibilityScanResults.violations).toEqual([]);
    });

    test('clients page should not have any automatically detectable accessibility issues', async ({ page }) => {
        await page.goto('#clients');

        const accessibilityScanResults = await new AxeBuilder({ page }).analyze();

        expect(accessibilityScanResults.violations).toEqual([]);
    });

    test('docs page should not have any automatically detectable accessibility issues', async ({ page }) => {
        await page.goto('#architecture');

        const accessibilityScanResults = await new AxeBuilder({ page }).analyze();

        expect(accessibilityScanResults.violations).toEqual([]);
    });
});

test.afterAll(async () => {
    stopBackend(backendProcess);
    backendProcess = undefined;
});
