import { test, expect } from '@playwright/test';
import { startBackend, stopBackend, configs, expand_and_sanitize_host_install, getTestPort } from './test-utils';
import { ChildProcess } from 'node:child_process';

test.describe('main page(s)', () => {
    let backendProcess: ChildProcess | undefined;

    test.beforeAll(async () => {
        backendProcess = await startBackend(configs["hosts-and-clients"]);
    });

    test.afterAll(async () => {
        stopBackend(backendProcess);
        backendProcess = undefined;
    });

    test('hosts', async ({ page }) => {
        await page.goto('#hosts');
        await page.waitForLoadState('networkidle');
        await page.waitForSelector('#hosts-tab', { state: 'visible' });
        await expect(page.locator('body')).toHaveScreenshot(`at_hosts.png`);
    });

    test('expanded host install', async ({ page }) => {
        await expand_and_sanitize_host_install(page);
        await page.waitForLoadState('networkidle');
        // Snapshot the expanded install section (header label + content) to keep the snapshot focused and stable
        await expect(page.locator('section[aria-labelledby="host-install-title"]')).toHaveScreenshot(`at_hosts_expanded_install.png`);
    });

    test('clients', async ({ page }) => {
        await page.goto('#clients');
        await page.waitForLoadState('networkidle');
        await page.waitForSelector('#clients-tab', { state: 'visible' });
        await expect(page.locator('body')).toHaveScreenshot(`at_clients.png`);
    });

    test('platform support', async ({ page }) => {
        await page.goto('#architecture');
        await page.waitForLoadState('networkidle');
        await page.waitForSelector('#platform-support-title', { state: 'visible' });
        await expect(page.locator('section[aria-labelledby="platform-support-title"]')).toHaveScreenshot(`platform_support.png`);
    });
});

test.describe('token login', () => {
    let backendProcess: ChildProcess | undefined;

    test.beforeAll(async () => {
        backendProcess = await startBackend(configs["auth-token"], true);
    });

    test.afterAll(async () => {
        stopBackend(backendProcess);
        backendProcess = undefined;
    });

    test('login page', async ({ page }) => {
        // Use HTTPS for TLS-enabled configs
        const port = getTestPort();
        await page.goto(`https://127.0.0.1:${port}/login`);
        await page.waitForLoadState('networkidle');
        await expect(page.locator('body')).toHaveScreenshot(`login_token_auth.png`);
    });

    test('login page - session expired', async ({ page }) => {
        // Use HTTPS for TLS-enabled configs
        const port = getTestPort();
        await page.goto(`https://127.0.0.1:${port}/login?error=session_expired`);
        await page.waitForSelector('.alert-warning', { state: 'visible' });
        await page.waitForLoadState('networkidle');
        await expect(page.locator('#main-content')).toHaveScreenshot(`login_token_session_expired.png`);
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

    test('login page', async ({ page }) => {
        // Use HTTPS for TLS-enabled configs
        const port = getTestPort();
        await page.goto(`https://127.0.0.1:${port}/login`);
        await page.waitForLoadState('networkidle');
        await expect(page.locator('body')).toHaveScreenshot(`login_oidc_auth.png`);
    });

    test('login page - session expired', async ({ page }) => {
        // Use HTTPS for TLS-enabled configs
        const port = getTestPort();
        await page.goto(`https://127.0.0.1:${port}/login?error=session_expired`);
        await page.waitForSelector('.alert-warning', { state: 'visible' });
        await page.waitForLoadState('networkidle');
        await expect(page.locator('#main-content')).toHaveScreenshot(`login_oidc_session_expired.png`);
    });
});

test.describe('demo mode', () => {
    let backendProcess: ChildProcess | undefined;

    test.beforeAll(async () => {
        backendProcess = await startBackend(undefined, false, 'demo-service');
    });

    test.afterAll(async () => {
        stopBackend(backendProcess);
        backendProcess = undefined;
    });

    test('main page', async ({ page }) => {
        await page.goto(`/`);
        await page.waitForLoadState('networkidle');
        await expect(page.locator('body')).toHaveScreenshot(`demo_main_page.png`);
    });
});
