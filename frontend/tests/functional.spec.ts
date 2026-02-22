import fs from 'node:fs';
import path from 'node:path';
import { test, expect } from '@playwright/test';
import { getBaseUrl } from './test-utils';
import { CONFIG_KEYS, configPathForKey } from './backend-utils';

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

// The repository keeps a directory of configuration files that are used to
// start various coordinator backends during tests.  A mismatch between
// `CONFIG_KEYS` (which is statically  provided for improved TS inference) 
// and the actual files can easily creep in when new features
// are added or old ones removed.
//
// This spec asserts set-equivalence between `CONFIG_KEYS` and the actual files. It also double-checks
// that `configPathForKey` points at a real file for every key.
test('CONFIG_KEYS matches configs directory', async () => {
    const thisDir = path.dirname(new URL(import.meta.url).pathname);
    const cfgDir = path.resolve(thisDir, 'configs');
    const files = fs.readdirSync(cfgDir).filter((f) => f.endsWith('.toml'));
    const keysFromFiles = files.map((f) => f.replace(/\.toml$/, ''));

    expect(new Set(keysFromFiles)).toEqual(new Set(CONFIG_KEYS));

    for (const key of CONFIG_KEYS) {
        const p = configPathForKey(key);
        expect(fs.existsSync(p)).toBe(true);
    }
});
