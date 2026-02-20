import { test, expect } from '@playwright/test';
import { configs, getBaseUrl } from './test-utils';

test.describe('token login', () => {
    const cfg = configs["auth-token"];

    test('redirects unauthorized access to login', async ({ page }) => {
        const base = getBaseUrl(cfg, true);
        // Try to access the main page without authentication
        await page.goto(`${base}/`);
        await page.waitForLoadState('networkidle');
        // Should redirect to login page
        await expect(page).toHaveURL(`${base}/login`);
        await expect(page.locator('#login-title')).toContainText('Sign in');
    });

    test('full login and logout', async ({ page }) => {
        const base = getBaseUrl(cfg, true);
        await page.goto(`${base}/login`);
        await page.waitForLoadState('networkidle');
        await page.fill('#token', 'testtoken');
        await page.click('button:has-text("Login")');
        await page.waitForLoadState('networkidle');
        await expect(page).toHaveURL(`${base}/`);
        await page.click('button[aria-label="Logout"]');
        await page.waitForLoadState('networkidle');
        await expect(page).toHaveURL(`${base}/login`);
    });
});

test.describe('OIDC login', () => {
    const cfg = configs["auth-oidc"];

    test('redirects unauthorized access to login', async ({ page }) => {
        const base = getBaseUrl(cfg, true);
        // Try to access the main page without authentication
        await page.goto(`${base}/`);
        await page.waitForLoadState('networkidle');
        // Should redirect to login page
        await expect(page).toHaveURL(`${base}/login`);
        await expect(page.locator('#login-title')).toContainText('Sign in');
    });
});