import { test, expect } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright'; // 1
import { expand_and_sanitize_host_install, getBaseUrl } from './test-utils';



test.describe('main page(s)', () => {
    const base = getBaseUrl('hosts-and-clients');

    test('hosts page should not have any automatically detectable accessibility issues', async ({ page }) => {
        await page.goto(base + '#hosts');
        await expand_and_sanitize_host_install(page, 'hosts-and-clients');

        const accessibilityScanResults = await new AxeBuilder({ page }).analyze();

        expect(accessibilityScanResults.violations).toEqual([]);
    });

    test('clients page should not have any automatically detectable accessibility issues', async ({ page }) => {
        await page.goto(base + '#clients');

        const accessibilityScanResults = await new AxeBuilder({ page }).analyze();

        expect(accessibilityScanResults.violations).toEqual([]);
    });

    test('docs page should not have any automatically detectable accessibility issues', async ({ page }) => {
        await page.goto(base + '#architecture');

        const accessibilityScanResults = await new AxeBuilder({ page }).analyze();

        expect(accessibilityScanResults.violations).toEqual([]);
    });
});

