import { expect, test } from '@playwright/test';
import { getBaseUrl, sanitizeVersion } from './test-utils';

// This test is mobile-specific. Desktop projects should ignore this file via Playwright config.
test.describe('mobile navigation', () => {
    const base = getBaseUrl('nada');

    test('mobile navigation opens and shows backdrop', async ({ page }) => {
        await page.goto(`${base}/`);
        // Click the visible hamburger label (aria-label="Toggle menu")
        await page.waitForSelector('button[aria-label="Toggle menu"]');
        await page.click('button[aria-label="Toggle menu"]');
        // Wait for the mobile menu backdrop or nav tabs to appear
        await page.waitForSelector('.mobile-menu-backdrop, header .nav-tabs', {
            state: 'visible',
        });
        await page.waitForLoadState('networkidle');

        // Redact version for stable snapshots
        await sanitizeVersion(page);

        await expect(page).toHaveScreenshot(`mobile-navigation.png`);
    });

    test('ARIA snapshot for mobile navigation', async ({ page }, testInfo) => {
        // biome-ignore lint/suspicious/noSkippedTests: intentionally skipped on non-Mobile-Light projects where the test is not applicable
        test.skip(
            testInfo.project.name !== 'Mobile Light',
            "Theme doesn't show in ARIA snapshots",
        );
        await page.goto(`${base}/`);
        // Click the hamburger button (aria-label="Toggle menu")
        await page.waitForSelector('button[aria-label="Toggle menu"]');
        await page.click('button[aria-label="Toggle menu"]');
        // Wait for the mobile menu backdrop or nav tabs to appear
        await page.waitForSelector('.mobile-menu-backdrop, header .nav-tabs', {
            state: 'visible',
        });
        await page.waitForLoadState('networkidle');

        // Redact version for stable snapshots
        await sanitizeVersion(page);

        await expect(page.locator('body')).toMatchAriaSnapshot({
            name: 'mobile-navigation.aria.yml',
        });
    });
});
