import { test, expect } from '@playwright/test';
import { startBackend, stopBackend, configs, screenshotOpts } from './test-utils';

let backendProcess: any | undefined;

// This test is mobile-specific. Desktop projects should ignore this file via Playwright config.
test.describe('mobile navigation', () => {
    test.beforeAll(async () => {
        // Use the hosts-and-clients config which shows the full navigation
        backendProcess = await startBackend(configs['nada']);
    });

    test('mobile navigation opens and shows backdrop', async ({ page, browserName }) => {
        await page.goto('/');
        // Click the visible hamburger label (aria-label="Toggle menu")
        await page.waitForSelector('label[for="mobile-menu-toggle"], label[aria-label="Toggle menu"]');
        await page.click('label[for="mobile-menu-toggle"], label[aria-label="Toggle menu"]');
        // Wait for the mobile menu backdrop or nav tabs to appear
        await page.waitForSelector('.mobile-menu-backdrop, header .nav-tabs', { state: 'visible' });
        // Stabilize animations
        await page.emulateMedia({ reducedMotion: 'reduce' });
        await page.waitForLoadState('networkidle');

        await expect(page).toHaveScreenshot(`mobile-navigation.png`, screenshotOpts);
    });

    test.afterAll(async () => {
        stopBackend(backendProcess);
        backendProcess = undefined;
    });
});
