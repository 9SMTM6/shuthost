// filepath: frontend/tests/pwa-installability.spec.ts
import { test, expect } from '@playwright/test';
import { configs, getBaseUrl } from './test-utils';


 test('PWA install prompt is available', async ({ page }) => {
    test.skip(true, "TODO, this doesnt seem to work correctly.");
    await page.goto(getBaseUrl(configs["hosts-and-clients"]) + '/');
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

 test('PWA manifest and icons meet heuristics', async ({ page, request }) => {
    // Heuristics taken from https://web.dev/articles/install-criteria, 
    // and from my experience (regarding image purpose)

    // Navigate to the app root so relative URLs resolve correctly
    await page.goto(getBaseUrl(configs["hosts-and-clients"]) + '/');

    // Find manifest link
    const manifestHref = await page.locator('link[rel="manifest"]').getAttribute('href');
    expect(manifestHref).toBeTruthy();

    const manifestUrl = new URL(manifestHref!, page.url()).toString();
    const manifestResp = await request.get(manifestUrl);
    expect(manifestResp.status()).toBe(200);

    const manifest = await manifestResp.json();

    // Basic required fields
    expect(manifest.name || manifest.short_name).toBeTruthy();
    expect(manifest.start_url).toBeTruthy();

    // display must be one of the allowed values
    const allowedDisplays = ['fullscreen', 'standalone', 'minimal-ui', 'window-controls-overlay'];
    expect(allowedDisplays.includes(manifest.display)).toBe(true);

    // prefer_related_applications must not be present or must be false
    expect(manifest.prefer_related_applications === undefined || manifest.prefer_related_applications === false).toBe(true);

    // Icons: must include a 192px and a 512px icon
    expect(Array.isArray(manifest.icons)).toBe(true);
    expect(manifest.icons.length).toBeGreaterThan(0);

    let has192 = false;
    let has512 = false;

    for (const icon of manifest.icons) {
        expect(icon.src).toBeTruthy();

        // purpose: either absent or includes 'any'
        if (icon.purpose !== undefined) {
            const purposes = String(icon.purpose).split(/\s+/);
            expect(purposes.includes('any')).toBe(true);
        }

        // Check sizes attribute for 192x192 and 512x512
        const sizes = (icon.sizes || '').toString().split(/\s+/).filter(Boolean);
        for (const s of sizes) {
            const m = s.match(/^(\d+)x(\d+)$/);
            if (m) {
                const w = parseInt(m[1], 10);
                if (w === 192) has192 = true;
                if (w === 512) has512 = true;
            }
        }

        const iconUrl = new URL(icon.src, manifestUrl).toString();
        const iconResp = await request.get(iconUrl);
        expect(iconResp.status()).toBeLessThan(400);
        const ct = (iconResp.headers()['content-type'] || '').toLowerCase();
        expect(ct.startsWith('image/')).toBe(true);

        // If sizes didn't explicitly include the required sizes, probe the image in the browser to get natural dimensions
        if (!has192 || !has512) {
            const dims = await page.evaluate(async (src) => {
                return new Promise<{w:number,h:number}>((resolve, reject) => {
                    const img = new Image();
                    img.onload = () => resolve({ w: img.naturalWidth, h: img.naturalHeight });
                    img.onerror = () => reject(new Error('failed to load image'));
                    img.src = src;
                });
            }, iconUrl).catch(() => ({ w: 0, h: 0 }));

            if (dims.w === 192 || dims.h === 192) has192 = true;
            if (dims.w === 512 || dims.h === 512) has512 = true;
        }
    }

    expect(has192).toBe(true);
    expect(has512).toBe(true);
});

