import { test, expect } from '@playwright/test';
import { getBaseUrl } from './test-utils';

test.describe('token login', () => {
    const base = getBaseUrl('auth-token', true);

    test('redirects unauthorized access to login', async ({ page }) => {
        // Try to access the main page without authentication
        await page.goto(`${base}/`);
        await page.waitForLoadState('networkidle');
        // Should redirect to login page
        await expect(page).toHaveURL(`${base}/login`);
        await expect(page.locator('#login-title')).toContainText('Sign in');
    });

    test('full login and logout', async ({ page }) => {
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
    const base = getBaseUrl("auth-oidc", true);

    test('redirects unauthorized access to login', async ({ page }) => {
        // Try to access the main page without authentication
        await page.goto(`${base}/`);
        await page.waitForLoadState('networkidle');
        // Should redirect to login page
        await expect(page).toHaveURL(`${base}/login`);
        await expect(page.locator('#login-title')).toContainText('Sign in');
    });
});