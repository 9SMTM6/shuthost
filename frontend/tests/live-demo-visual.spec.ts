import { expect, test } from '@playwright/test';
import { sanitizeVersion } from './test-utils';

const BASE_URL = process.env['DEMO_URL'] ?? 'https://9smtm6.github.io/shuthost';

test('main page (hosts tab)', async ({ page }) => {
    await page.goto(`${BASE_URL}/`);
    await page.waitForLoadState('networkidle');
    await sanitizeVersion(page);
    await expect(page.locator('body')).toHaveScreenshot('live-demo-main.png');
});

test('docs/architecture tab', async ({ page }) => {
    await page.goto(`${BASE_URL}/#architecture`);
    await page.waitForLoadState('networkidle');
    await sanitizeVersion(page);
    await expect(page.locator('body')).toHaveScreenshot('live-demo-architecture.png');
});
