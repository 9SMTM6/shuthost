import { expect, test } from '@playwright/test';

const BASE_URL = process.env['DEMO_URL'] ?? 'https://9smtm6.github.io/shuthost';

test('main page (hosts tab)', async ({ page }) => {
    await page.goto(`${BASE_URL}/`);
    await page.waitForLoadState('networkidle');
    await expect(page.locator('#host-table-body')).toBeVisible();
    await expect(page.locator('#demo-mode-disclaimer')).toBeVisible();
});

test('docs/architecture tab', async ({ page }) => {
    await page.goto(`${BASE_URL}/docs`);
    await page.waitForLoadState('networkidle');
    await expect(page.locator('#platform-support-title')).toBeVisible();
});
