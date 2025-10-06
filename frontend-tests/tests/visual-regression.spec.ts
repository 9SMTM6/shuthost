import { test, expect } from '@playwright/test';
import { startBackend, stopBackend, configs, screenshotOpts } from './test-utils';

test.describe('visual regression', () => {
    let backendProcess: any | undefined;

    test.beforeAll(async () => {
        backendProcess = await startBackend(configs["hosts-and-clients"]);
    });

    test.afterAll(async () => {
        stopBackend(backendProcess);
        backendProcess = undefined;
    });

    test('hosts', async ({ page }) => {
        await page.goto('#hosts');
        await page.emulateMedia({ reducedMotion: 'reduce' });
        await page.waitForLoadState('networkidle');
        await page.waitForSelector('#main-content', { state: 'attached' });
        await expect(page.locator('body')).toHaveScreenshot(`at_hosts.png`, screenshotOpts);
    });

    test('clients', async ({ page }) => {
        await page.goto('#clients');
        await page.emulateMedia({ reducedMotion: 'reduce' });
        await page.waitForLoadState('networkidle');
        await page.waitForSelector('#main-content', { state: 'attached' });
        await expect(page.locator('body')).toHaveScreenshot(`at_clients.png`, screenshotOpts);
    });
});
