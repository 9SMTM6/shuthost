// filepath: frontend/tests/pwa-installability.spec.ts
import { test, expect } from '@playwright/test';
import { configs, startBackend, stopBackend } from './test-utils';

let backendProcess: any | undefined;

test.beforeAll(async () => {
    backendProcess = await startBackend(configs["hosts-and-clients"]);
});

test('PWA install prompt is available', async ({ page }) => {
    test.skip(true, "TODO, this doesnt seem to work correctly. We could test for heuristics (e.g. that manifest and icons are reachable, display:standalone, that all icons have purpose:any, etc.");
    await page.goto('/');
    const installPromptFired = page.evaluate(() => {
        return new Promise<boolean>((resolve) => {
            window.addEventListener('beforeinstallprompt', () => resolve(true), { once: true });
            // Timeout after 5 seconds if not fired
            setTimeout(() => resolve(false), 5000);
        });
    });

    const isInstallable = await installPromptFired;
    expect(isInstallable).toBe(true);
});

test.afterAll(async () => {
    stopBackend(backendProcess);
    backendProcess = undefined;
});
