import { test, expect } from '@playwright/test';
import { getBaseUrl, expand_and_sanitize_host_install, sanitizeEnvironmentDependents } from './test-utils';

// hosts-only and hosts-and-clients tests
for (const name of ['hosts-only', 'hosts-and-clients'] as const) {
  test.describe(`${name} config`, () => {
    const base = getBaseUrl(name);
    test(`ARIA snapshot for hosts tab (${name})`, async ({ page }) => {
      await page.goto(base + '#hosts');
      await page.waitForSelector('#host-table-body', { state: 'attached' });
      await expect(page.locator('#main-content')).toMatchAriaSnapshot({ name: `at_hosts-cfg_${name}.aria.yml` });
    });

    test(`ARIA snapshot for clients tab (${name})`, async ({ page }) => {
      await page.goto(base + '#clients');
      await page.waitForSelector('#client-table-body', { state: 'attached' });
      await expect(page.locator('#main-content')).toMatchAriaSnapshot({ name: `at_clients-cfg_${name}.aria.yml` });
    });
  });
}

// Test for no-db config to ensure Last Used column is hidden
test.describe('no-db config', () => {
  const base = getBaseUrl('no-db');
  test('ARIA snapshot for clients table (no-db)', async ({ page }) => {
    await page.goto(base + '#clients');
    await page.waitForSelector('#client-table-body', { state: 'attached' });
    await expect(page.locator('#client-table-body')).toMatchAriaSnapshot({ name: `at_clients-table-cfg_no-db.aria.yml` });
  });
});

// Architecture tab is independent of config
test.describe('architecture tab', () => {
  const base = getBaseUrl('hosts-and-clients');
  test('ARIA snapshot for architecture tab', async ({ page }) => {
    await page.goto(base + '#architecture');
    await page.waitForSelector('#architecture-tab', { state: 'visible' });
    await expect(page.locator('#main-content')).toMatchAriaSnapshot({ name: `at_architecture.aria.yml` });
  });
});

// Additional test: expanded Install sections for the 'nada' config
test.describe('expanded install panels', () => {
  const key = 'nada';
  const base = getBaseUrl(key);
  test('ARIA snapshot with Install Host Agent expanded (nada)', async ({ page }) => {
    await page.goto(base + '#hosts');
    await expand_and_sanitize_host_install(page, key);
    await expect(page.locator('#main-content')).toMatchAriaSnapshot({ name: `cfg_${key}-at_hosts-expanded_install.aria.yml` });
  });
  test('ARIA snapshot with Install Client expanded (nada)', async ({ page }) => {
    await page.goto(base + '#clients');
    await page.waitForSelector('#client-install-header');
    await page.click('#client-install-header');
    await page.waitForSelector('#client-install-content', { state: 'visible' });
    await sanitizeEnvironmentDependents(page);
    await expect(page.locator('#main-content')).toMatchAriaSnapshot({ name: `cfg_${key}-at_clients-expanded_install.aria.yml` });
  });
});

test.describe('token login', () => {
  const base = getBaseUrl('auth-token', true);
  test('ARIA snapshot for login page (token)', async ({ page }) => {
    await page.goto(base + '/login');
    await page.waitForLoadState('networkidle');
    await expect(page.locator('#main-content')).toMatchAriaSnapshot({ name: 'login_token.aria.yml' });
  });
});

test.describe('OIDC login', () => {
  test('ARIA snapshot for login page (OIDC)', async ({ page }) => {
    const base = getBaseUrl('auth-oidc', true);
    await page.goto(base + '/login');
    await page.waitForLoadState('networkidle');
    await expect(page.locator('#main-content')).toMatchAriaSnapshot({ name: 'login_oidc.aria.yml' });
  });
});

test.describe('no-auth landing page', () => {
  const base = getBaseUrl('auth-none');
  test('ARIA snapshot of root page (no-auth)', async ({ page }) => {
    await page.goto(base + '/');
    await page.waitForSelector('#main-content', { state: 'attached' });
    await expect(page.locator('#main-content')).toMatchAriaSnapshot({ name: `cfg_no-auth-root.aria.yml` });
  });
  test('ARIA snapshot with security config expanded (no-auth)', async ({ page }) => {
    await page.goto(base + '/');
    await page.waitForSelector('#main-content', { state: 'attached' });
    await page.click('#security-config-header');
    await page.waitForSelector('#security-config-content', { state: 'visible' });
    await sanitizeEnvironmentDependents(page);
    await expect(page.locator('#main-content')).toMatchAriaSnapshot({ name: 'cfg_no-auth-root-expanded-security.aria.yml' });
  });
});

test.describe('auth-outdated-exceptions landing page', () => {
  const base = getBaseUrl('auth-outdated-exceptions');
  test('ARIA snapshot of security config (auth-outdated-exceptions)', async ({ page }) => {
    await page.goto(base + '/');
    await page.waitForSelector('#main-content', { state: 'attached' });
    await expect(page.locator('#main-content')).toMatchAriaSnapshot({ name: `cfg_auth-outdated-exceptions-root.aria.yml` });
    await page.click('#security-config-header');
    await page.waitForSelector('#security-config-content', { state: 'visible' });
    await sanitizeEnvironmentDependents(page);
    await expect(page.locator('#security-config-content')).toMatchAriaSnapshot({ name: 'cfg_auth-outdated-exceptions-expanded-security.aria.yml' });
  });
});
