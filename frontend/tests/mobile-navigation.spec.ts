import { expect, test } from '@playwright/test';
import { getBaseUrl, sanitizeEnvironmentDependents, sanitizeVersion } from './test-utils';

test.describe('test harness', () => {
    const base = getBaseUrl('hosts-only');
    test("test harness enables mobile css media queries", async ({ page }) => {
        await page.goto(`${base}/hosts/archive`);
        // Check JS queries
        const hoverNone = await page.evaluate(() => window.matchMedia('(hover: none)').matches);
        expect(hoverNone).toBe(true);
        const pointerCoarse = await page.evaluate(() => window.matchMedia('(pointer: coarse)').matches);
        expect(pointerCoarse).toBe(true);

        // Check visibility of at least one touch-description element
        await expect(page.locator('.touch-description').first()).toBeVisible();

        await sanitizeEnvironmentDependents(page);
        await sanitizeVersion(page);

        await page.waitForLoadState('networkidle');
        
        await expect(page).toHaveScreenshot(`mobile-css-media-queries.png`);
        await expect(page.locator(`#main-content`)).toHaveScreenshot(`mobile-css-media-queries2.png`);
        await expect(page).toHaveScreenshot(`mobile-css-media-queries-now-different.png`);
        // fails now
        await expect(page.locator('.touch-description').first()).toBeVisible();
        await expect(page.locator(`body`)).toHaveScreenshot(`mobile-css-media-queries3.png`);
        await expect(page.locator(`main`).first()).toHaveScreenshot(`mobile-css-media-queries4.png`);
        // await expect(page.locator('.touch-description').first()).toHaveScreenshot('touch-description.png');
    });
});

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
