import { test, expect } from '@playwright/test';
import { configs, getBaseUrl, expand_and_sanitize_host_install, sanitizeEnvironmentDependents } from './test-utils';

// hosts-only and hosts-and-clients tests
for (const name of ['hosts-only', 'hosts-and-clients'] as const) {
  const path = configs[name];
  test.describe(`${name} config`, () => {
    test(`ARIA snapshot for hosts tab (${name})`, async ({ page }) => {
      await page.goto(getBaseUrl(path) + '#hosts');
      await page.waitForSelector('#host-table-body', { state: 'attached' });
      await expect(page.locator('#main-content')).toMatchAriaSnapshot({ name: `at_hosts-cfg_${name}.aria.yml` });
    });

    test(`ARIA snapshot for clients tab (${name})`, async ({ page }) => {
      await page.goto(getBaseUrl(path) + '#clients');
      await page.waitForSelector('#client-table-body', { state: 'attached' });
      await expect(page.locator('#main-content')).toMatchAriaSnapshot({ name: `at_clients-cfg_${name}.aria.yml` });
    });
  });
}

// Test for no-db config to ensure Last Used column is hidden
test.describe('no-db config', () => {
  const path = configs['no-db'];
  test('ARIA snapshot for clients table (no-db)', async ({ page }) => {
    await page.goto(getBaseUrl(path) + '#clients');
    await page.waitForSelector('#client-table-body', { state: 'attached' });
    await expect(page.locator('#client-table-body')).toMatchAriaSnapshot({ name: `at_clients-table-cfg_no-db.aria.yml` });
  });
});

// Architecture tab is independent of config
test.describe('architecture tab', () => {
  const path = configs['hosts-and-clients'];
  test('ARIA snapshot for architecture tab', async ({ page }) => {
    await page.goto(getBaseUrl(path) + '#architecture');
    await page.waitForSelector('#architecture-tab', { state: 'visible' });
    await expect(page.locator('#main-content')).toMatchAriaSnapshot({ name: `at_architecture.aria.yml` });
  });
});

// Additional test: expanded Install sections for the 'nada' config
test.describe('expanded install panels', () => {
  const path = configs['nada'];
  test('ARIA snapshot with Install Host Agent expanded (nada)', async ({ page }) => {
    await page.goto(getBaseUrl(path) + '#hosts');
    await expand_and_sanitize_host_install(page, path);
    await expect(page.locator('#main-content')).toMatchAriaSnapshot({ name: `cfg_nada-at_hosts-expanded_install.aria.yml` });
  });
  test('ARIA snapshot with Install Client expanded (nada)', async ({ page }) => {
    await page.goto(getBaseUrl(path) + '#clients');
    await page.waitForSelector('#client-install-header');
    await page.click('#client-install-header');
    await page.waitForSelector('#client-install-content', { state: 'visible' });
    await sanitizeEnvironmentDependents(page);
    await expect(page.locator('#main-content')).toMatchAriaSnapshot({ name: `cfg_nada-at_clients-expanded_install.aria.yml` });
  });
});

test.describe('token login', () => {
  const path = configs['auth-token'];
  test('ARIA snapshot for login page (token)', async ({ page }) => {
    await page.goto(getBaseUrl(path, true) + '/login');
    await page.waitForLoadState('networkidle');
    await expect(page.locator('#main-content')).toMatchAriaSnapshot({ name: 'login_token.aria.yml' });
  });
});

test.describe('OIDC login', () => {
  const path = configs['auth-oidc'];
  test('ARIA snapshot for login page (OIDC)', async ({ page }) => {
    await page.goto(getBaseUrl(path, true) + '/login');
    await page.waitForLoadState('networkidle');
    await expect(page.locator('#main-content')).toMatchAriaSnapshot({ name: 'login_oidc.aria.yml' });
  });
});

test.describe('no-auth landing page', () => {
  const path = configs['auth-none'];
  test('ARIA snapshot of root page (no-auth)', async ({ page }) => {
    await page.goto(getBaseUrl(path) + '/');
    await page.waitForSelector('#main-content', { state: 'attached' });
    await expect(page.locator('#main-content')).toMatchAriaSnapshot({ name: `cfg_no-auth-root.aria.yml` });
  });
  test('ARIA snapshot with security config expanded (no-auth)', async ({ page }) => {
    await page.goto(getBaseUrl(path) + '/');
    await page.waitForSelector('#main-content', { state: 'attached' });
    await page.click('#security-config-header');
    await page.waitForSelector('#security-config-content', { state: 'visible' });
    await sanitizeEnvironmentDependents(page);
    await expect(page.locator('#main-content')).toMatchAriaSnapshot({ name: 'cfg_no-auth-root-expanded-security.aria.yml' });
  });
});

test.describe('auth-outdated-exceptions landing page', () => {
  const path = configs['auth-outdated-exceptions'];
  test('ARIA snapshot of security config (auth-outdated-exceptions)', async ({ page }) => {
    await page.goto(getBaseUrl(path) + '/');
    await page.waitForSelector('#main-content', { state: 'attached' });
    await expect(page.locator('#main-content')).toMatchAriaSnapshot({ name: `cfg_auth-outdated-exceptions-root.aria.yml` });
    await page.click('#security-config-header');
    await page.waitForSelector('#security-config-content', { state: 'visible' });
    await sanitizeEnvironmentDependents(page);
    await expect(page.locator('#security-config-content')).toMatchAriaSnapshot({ name: 'cfg_auth-outdated-exceptions-expanded-security.aria.yml' });
  });
});
