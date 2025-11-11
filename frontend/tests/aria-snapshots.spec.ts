import { test, expect } from '@playwright/test';
import { startBackend, stopBackend, configs, expand_and_sanitize_host_install, getTestPort, sanitizeEnvironmentDependents } from './test-utils';
import { ChildProcess } from 'node:child_process';

let backendProcess: ChildProcess | undefined;

const hostsConfigs = ['hosts-only', 'hosts-and-clients'] as const;
for (const name of hostsConfigs) {
  const path = configs[name];
  test.describe(`${name} config`, () => {

    test.beforeAll(async () => {
      backendProcess = await startBackend(path);
    });

    test(`ARIA snapshot for hosts tab (${name})`, async ({ page }) => {
      await page.goto('#hosts');
      await page.waitForSelector('#host-table-body', { state: 'attached' });
      await expect(page.locator('#main-content')).toMatchAriaSnapshot({ name: `at_hosts-cfg_${name}.aria.yml` });
    });

    test(`ARIA snapshot for clients tab (${name})`, async ({ page }) => {
      await page.goto('#clients');
      await page.waitForSelector('#client-table-body', { state: 'attached' });
      await expect(page.locator('#main-content')).toMatchAriaSnapshot({ name: `at_clients-cfg_${name}.aria.yml` });
    });

    // Ensure we stop the backend for this config describe to avoid cross-describe state/port issues
    test.afterAll(async () => {
      stopBackend(backendProcess);
      backendProcess = undefined;
    });
  });
}

// Architecture tab is independent of config
test.describe('architecture tab', () => {
  test.beforeAll(async () => {
    backendProcess = await startBackend(configs["hosts-and-clients"]);
  });

  test('ARIA snapshot for architecture tab', async ({ page }) => {
    await page.goto('#architecture');
    await page.waitForSelector('#architecture-tab', { state: 'visible' });
    await expect(page.locator('#main-content')).toMatchAriaSnapshot({ name: `at_architecture.aria.yml` });
  });

  test.afterAll(async () => {
    stopBackend(backendProcess);
    backendProcess = undefined;
  });
});

// Additional test: expanded Install sections for the 'nada' config
test.describe('expanded install panels', () => {
  test.beforeAll(async () => {
    backendProcess = await startBackend(configs['nada']);
  });

  test('ARIA snapshot with Install Host Agent expanded (nada)', async ({ page }) => {
    await expand_and_sanitize_host_install(page);
    await expect(page.locator('#main-content')).toMatchAriaSnapshot({ name: `cfg_nada-at_hosts-expanded_install.aria.yml` });
  });

  test('ARIA snapshot with Install Client expanded (nada)', async ({ page }) => {
    await page.goto('#clients');
    // The checkbox input is hidden (CSS); click the visible header/label instead.
    await page.waitForSelector('#client-install-header');
    await page.click('#client-install-header');
    await page.waitForSelector('#client-install-content', { state: 'visible' });
    // Sanitize dynamic install command and config path for stable snapshots
    await sanitizeEnvironmentDependents(page);
    await expect(page.locator('#main-content')).toMatchAriaSnapshot({ name: `cfg_nada-at_clients-expanded_install.aria.yml` });
  });

  test.afterAll(async () => {
    stopBackend(backendProcess);
    backendProcess = undefined;
  });
});

// ARIA snapshots for login pages
test.describe('token login', () => {
  test.beforeAll(async () => {
    backendProcess = await startBackend(configs["auth-token"], true);
  });

  test.afterAll(async () => {
    stopBackend(backendProcess);
    backendProcess = undefined;
  });

  test('ARIA snapshot for login page (token)', async ({ page }) => {
    const port = getTestPort();
    await page.goto(`https://127.0.0.1:${port}/login`);
    await page.waitForLoadState('networkidle');
    await expect(page.locator('#main-content')).toMatchAriaSnapshot({ name: 'login_token.aria.yml' });
  });
});

test.describe('OIDC login', () => {
  test.beforeAll(async () => {
    backendProcess = await startBackend(configs["auth-oidc"], true);
  });

  test.afterAll(async () => {
    stopBackend(backendProcess);
    backendProcess = undefined;
  });

  test('ARIA snapshot for login page (OIDC)', async ({ page }) => {
    const port = getTestPort();
    await page.goto(`https://127.0.0.1:${port}/login`);
    await page.waitForLoadState('networkidle');
    await expect(page.locator('#main-content')).toMatchAriaSnapshot({ name: 'login_oidc.aria.yml' });
  });
});

// Snapshot the root page with the 'no-auth' config
test.describe('no-auth landing page', () => {
  test.beforeAll(async () => {
    backendProcess = await startBackend(configs['auth-none']);
  });

  test('ARIA snapshot of root page (no-auth)', async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('#main-content', { state: 'attached' });
    await expect(page.locator('#main-content')).toMatchAriaSnapshot({ name: `cfg_no-auth-root.aria.yml` });
  });

  test('ARIA snapshot with security config expanded (no-auth)', async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('#main-content', { state: 'attached' });
    // Expand the security config section
    await page.click('#security-config-header');
    await page.waitForSelector('#security-config-content', { state: 'visible' });
    // Sanitize dynamic config examples for stable snapshots
    await sanitizeEnvironmentDependents(page);
    await expect(page.locator('#main-content')).toMatchAriaSnapshot({ name: 'cfg_no-auth-root-expanded-security.aria.yml' });
  });

  test.afterAll(async () => {
    stopBackend(backendProcess);
    backendProcess = undefined;
  });
});

// Snapshot the root page with the 'auth-outdated-exceptions' config
test.describe('auth-outdated-exceptions landing page', () => {
  test.beforeAll(async () => {
    backendProcess = await startBackend(configs['auth-outdated-exceptions']);
  });

  test('ARIA snapshot of security config (auth-outdated-exceptions)', async ({ page }) => {
    await page.goto('/');
    await page.waitForSelector('#main-content', { state: 'attached' });
    // First snapshot: unexpanded security config
    await expect(page.locator('#main-content')).toMatchAriaSnapshot({ name: `cfg_auth-outdated-exceptions-root.aria.yml` });

    // Expand the security config section
    await page.click('#security-config-header');
    await page.waitForSelector('#security-config-content', { state: 'visible' });
    // Sanitize dynamic config examples for stable snapshots
    await sanitizeEnvironmentDependents(page);
    // Second snapshot: expanded security config
    await expect(page.locator('#security-config-content')).toMatchAriaSnapshot({ name: 'cfg_auth-outdated-exceptions-expanded-security.aria.yml' });
  });

  test.afterAll(async () => {
    stopBackend(backendProcess);
    backendProcess = undefined;
  });
});
