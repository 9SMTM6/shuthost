import { test, expect } from '@playwright/test';
import { configs, getBaseUrl, expand_and_sanitize_host_install } from './test-utils';

const mainCfg = configs["hosts-and-clients"];
test.describe('main page(s)', () => {
    test('hosts', async ({ page }) => {
        await page.goto(getBaseUrl(mainCfg) + '#hosts');
        await page.waitForLoadState('networkidle');
        await page.waitForSelector('#hosts-tab', { state: 'visible' });
        await expect(page.locator('body')).toHaveScreenshot(`at_hosts.png`);
    });

    test('expanded host install', async ({ page }) => {
        await page.goto(getBaseUrl(mainCfg) + '#hosts');
        await expand_and_sanitize_host_install(page, mainCfg);
        await page.waitForLoadState('networkidle');
        await expect(page.locator('section[aria-labelledby="host-install-title"]')).toHaveScreenshot(`at_hosts_expanded_install.png`);
    });

    test('clients', async ({ page }) => {
        await page.goto(getBaseUrl(mainCfg) + '#clients');
        await page.waitForLoadState('networkidle');
        await page.waitForSelector('#clients-tab', { state: 'visible' });
        await expect(page.locator('body')).toHaveScreenshot(`at_clients.png`);
    });

    test('platform support', async ({ page }) => {
        await page.goto(getBaseUrl(mainCfg) + '#architecture');
        await page.waitForLoadState('networkidle');
        await page.waitForSelector('#platform-support-title', { state: 'visible' });
        await expect(page.locator('section[aria-labelledby="platform-support-title"]')).toHaveScreenshot(`platform_support.png`);
    });

    test('javascript error display', async ({ page }) => {
        await page.goto(getBaseUrl(mainCfg) + '/');
        await page.waitForLoadState('networkidle');
        await page.evaluate(() => {
            setTimeout(() => {
                throw new Error('Test JavaScript error for visual regression');
            }, 0);
        });
        await page.waitForSelector('#js-error', { state: 'visible' });
        await expect(page.locator('#js-error')).toBeVisible();
        await expect(page.locator('#js-error')).toHaveScreenshot('js_error_display.png');
    });

    test('license table header', async ({ page }) => {
        await page.goto(getBaseUrl(mainCfg) + '/about');
        await page.waitForLoadState('networkidle');
        await page.waitForSelector('#dependencies-title', { state: 'visible' });
        await expect(page.locator('thead')).toHaveScreenshot(`license_table_header.png`);
    });

    test('GPL2 license display', async ({ page }) => {
        await page.goto(getBaseUrl(mainCfg) + '/about');
        await page.waitForLoadState('networkidle');
        await page.waitForSelector('#license-GPL-2\\.0-only', { state: 'visible' });
        await expect(page.locator('#license-GPL-2\\.0-only').locator('..')).toHaveScreenshot(`gpl2_license_display.png`);
    });
});

test.describe('token login', () => {
    const cfg = configs["auth-token"];

    test('login page', async ({ page }) => {
        await page.goto(getBaseUrl(cfg, true) + '/login');
        await page.waitForLoadState('networkidle');
        await expect(page.locator('body')).toHaveScreenshot(`login_token_auth.png`);
    });

    test('login page - session expired', async ({ page }) => {
        await page.goto(getBaseUrl(cfg, true) + '/login?error=session_expired');
        await page.waitForSelector('.alert-warning', { state: 'visible' });
        await page.waitForLoadState('networkidle');
        await expect(page.locator('#main-content')).toHaveScreenshot(`login_token_session_expired.png`);
    });

    test('noscript warning when JS disabled', async ({ browser }) => {
        const context = await browser.newContext({ javaScriptEnabled: false });
        const page = await context.newPage();
        await page.goto(getBaseUrl(cfg, true) + '/login');
        await page.waitForLoadState('networkidle');
        await expect(page.locator('#noscript-warning')).toBeVisible();
        await expect(page.locator('#noscript-warning')).toHaveScreenshot('noscript_warning.png');
        await context.close();
    });
});

test.describe('OIDC login', () => {
    const cfg = configs["auth-oidc"];

    test('login page', async ({ page }) => {
        await page.goto(getBaseUrl(cfg, true) + '/login');
        await page.waitForLoadState('networkidle');
        await expect(page.locator('body')).toHaveScreenshot(`login_oidc_auth.png`);
    });

    test('login page - session expired', async ({ page }) => {
        await page.goto(getBaseUrl(cfg, true) + '/login?error=session_expired');
        await page.waitForSelector('.alert-warning', { state: 'visible' });
        await page.waitForLoadState('networkidle');
        await expect(page.locator('#main-content')).toHaveScreenshot(`login_oidc_session_expired.png`);
    });
});

test.describe('demo mode', () => {
    test('main page', async ({ page }) => {
        await page.goto(getBaseUrl(undefined) + '/');
        await page.waitForLoadState('networkidle');
        await expect(page.locator('body')).toHaveScreenshot(`demo_main_page.png`);
    });
});
