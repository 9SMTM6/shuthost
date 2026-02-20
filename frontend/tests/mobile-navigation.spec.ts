import { test, expect } from '@playwright/test';
import { configs, getBaseUrl } from './test-utils';

// This test is mobile-specific. Desktop projects should ignore this file via Playwright config.
test.describe('mobile navigation', () => {
    const cfg = configs['nada'];

    test('mobile navigation opens and shows backdrop', async ({ page }) => {
        await page.goto(getBaseUrl(cfg) + '/');
        // Click the visible hamburger label (aria-label="Toggle menu")
        await page.waitForSelector('label[for="mobile-menu-toggle"], label[aria-label="Toggle menu"]');
        await page.click('label[for="mobile-menu-toggle"], label[aria-label="Toggle menu"]');
        // Wait for the mobile menu backdrop or nav tabs to appear
        await page.waitForSelector('.mobile-menu-backdrop, header .nav-tabs', { state: 'visible' });
        await page.waitForLoadState('networkidle');

        await expect(page).toHaveScreenshot(`mobile-navigation.png`);
    });

    test('ARIA snapshot for mobile navigation', async ({ page }, testInfo) => {
        test.skip(testInfo.project.name !== 'Mobile Light', "Theme doesn't show in ARIA snapshots");
        await page.goto(getBaseUrl(cfg) + '/');
        // Click the visible hamburger label (aria-label="Toggle menu")
        await page.waitForSelector('label[for="mobile-menu-toggle"], label[aria-label="Toggle menu"]');
        await page.click('label[for="mobile-menu-toggle"], label[aria-label="Toggle menu"]');
        // Wait for the mobile menu backdrop or nav tabs to appear
        await page.waitForSelector('.mobile-menu-backdrop, header .nav-tabs', { state: 'visible' });
        await page.waitForLoadState('networkidle');

        // Redact version for stable snapshots
        await page.evaluate(() => {
            const footer = document.querySelector('footer');
            if (footer && footer.textContent) {
                footer.textContent = footer.textContent.replace(/ShutHost Coordinator v[\d.]+/, 'ShutHost Coordinator v<<VERSION>>');
            }
        });

        await expect(page.locator('body')).toMatchAriaSnapshot({ name: 'mobile-navigation.aria.yml' });
    });
});
