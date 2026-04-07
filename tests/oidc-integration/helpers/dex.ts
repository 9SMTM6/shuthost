import { type Page } from '@playwright/test';

export const DEX_EMAIL = 'testuser@example.com';
export const DEX_PASSWORD = 'password';

/**
 * Perform a full interactive OIDC login via Dex.
 *
 * Assumes the browser has already been navigated to a page that redirects
 * to the Dex login screen (or is already on it).
 *
 * Steps:
 *   1. Wait for the Dex login page (URL contains `/auth/` or `/dex/`).
 *   2. Fill in email and password.
 *   3. Submit the login form.
 *   4. Handle the optional grant/approval screen (if not skipped).
 *   5. Return when the browser lands back on `baseUrl`.
 */
export const loginWithDex = async (page: Page, baseUrl: string): Promise<void> => {
    // Wait until we are on the Dex UI (the URL will contain the Dex host)
    await page.waitForURL(/127\.0\.0\.1:5556/, { timeout: 15000 });

    // Dex login form — the exact selectors depend on the Dex version but these
    // are stable across v2.x releases.
    const loginInput = page.locator('input[name="login"], input[type="email"], input[id="login"]');
    const passwordInput = page.locator('input[name="password"], input[type="password"]');
    const submitButton = page.locator('button[type="submit"]');

    await loginInput.fill(DEX_EMAIL);
    await passwordInput.fill(DEX_PASSWORD);
    await submitButton.click();

    // Handle the optional grant-approval screen (skipApprovalScreen=true in our
    // config, but we defensively handle it anyway).
    try {
        const approveButton = page.locator('button[type="submit"]:has-text("Grant Access"), button[type="submit"]:has-text("Approve")');
        await approveButton.waitFor({ timeout: 3000 });
        await approveButton.click();
    } catch {
        // No approval screen — that's expected with skipApprovalScreen: true
    }

    // Wait until the browser has returned to the coordinator
    await page.waitForURL(new RegExp(baseUrl.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')), { timeout: 15000 });
};
