import { test, expect } from '@playwright/test';
import { startBackend, stopBackend, configs, getTestPort } from './test-utils';
import { ChildProcess } from 'node:child_process';

test.describe('token login', () => {
    let backendProcess: ChildProcess | undefined;

    test.beforeAll(async () => {
        backendProcess = await startBackend(configs["auth-token"], true);
    });

    test.afterAll(async () => {
        stopBackend(backendProcess);
        backendProcess = undefined;
    });

    test('redirects unauthorized access to login', async ({ page }) => {
        // Use HTTPS for TLS-enabled configs
        const port = getTestPort();
        // Try to access the main page without authentication
        await page.goto(`https://127.0.0.1:${port}/`);
        await page.waitForLoadState('networkidle');
        // Should redirect to login page
        await expect(page).toHaveURL(`https://127.0.0.1:${port}/login`);
        await expect(page.locator('h1')).toHaveText('Sign in');
    });

    test('full login and logout', async ({ page }) => {
        const port = getTestPort();
        await page.goto(`https://127.0.0.1:${port}/login`);
        await page.waitForLoadState('networkidle');
        await page.fill('#token', 'testtoken');
        await page.click('button:has-text("Login")');
        await page.waitForLoadState('networkidle');
        await expect(page).toHaveURL(`https://127.0.0.1:${port}/`);
        await page.click('button[aria-label="Logout"]');
        await page.waitForLoadState('networkidle');
        await expect(page).toHaveURL(`https://127.0.0.1:${port}/login`);
    });
});

test.describe('OIDC login', () => {
    let backendProcess: ChildProcess | undefined;

    test.beforeAll(async () => {
        backendProcess = await startBackend(configs["auth-oidc"], true);
    });

    test.afterAll(async () => {
        stopBackend(backendProcess);
        backendProcess = undefined;
    });

    test('redirects unauthorized access to login', async ({ page }) => {
        // Use HTTPS for TLS-enabled configs
        const port = getTestPort();
        // Try to access the main page without authentication
        await page.goto(`https://127.0.0.1:${port}/`);
        await page.waitForLoadState('networkidle');
        // Should redirect to login page
        await expect(page).toHaveURL(`https://127.0.0.1:${port}/login`);
        await expect(page.locator('h1')).toHaveText('Sign in');
    });
});