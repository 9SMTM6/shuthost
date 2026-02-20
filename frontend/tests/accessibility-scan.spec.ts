import { test, expect } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright'; // 1
import { configs, expand_and_sanitize_host_install, getBaseUrl } from './test-utils';



test.describe('main page(s)', () => {
    test('hosts page should not have any automatically detectable accessibility issues', async ({ page }) => {
        const cfg = configs['hosts-and-clients'];
        await page.goto(getBaseUrl(cfg) + '#hosts');
        await expand_and_sanitize_host_install(page, configs['hosts-and-clients']);

        const accessibilityScanResults = await new AxeBuilder({ page }).analyze();

        expect(accessibilityScanResults.violations).toEqual([]);
    });

    test('clients page should not have any automatically detectable accessibility issues', async ({ page }) => {
        const cfg = configs['hosts-and-clients'];
        await page.goto(getBaseUrl(cfg) + '#clients');

        const accessibilityScanResults = await new AxeBuilder({ page }).analyze();

        expect(accessibilityScanResults.violations).toEqual([]);
    });

    test('docs page should not have any automatically detectable accessibility issues', async ({ page }) => {
        const cfg = configs['hosts-and-clients'];
        await page.goto(getBaseUrl(cfg) + '#architecture');

        const accessibilityScanResults = await new AxeBuilder({ page }).analyze();

        expect(accessibilityScanResults.violations).toEqual([]);
    });
});

