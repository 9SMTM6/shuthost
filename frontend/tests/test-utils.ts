/// <reference lib="dom" />

import { Page } from '@playwright/test';
// child_process functions removed; not needed now
export {
  configs,
} from './backend-utils';
import { assignedPortForConfig} from './backend-utils';

// Return a complete base URL (including protocol and port) for a given
// coordinator identifier.  `configKey` should be one of the keys from
// `configs` (e.g. "hosts-only", "auth-token"), or the literal string
// `'demo'` for the special demo‑mode backend.  If omitted a per‑worker
// fallback port is used (only expected in legacy code).  `useTls` controls
// whether https:// is used; some configs (auth-token, auth-oidc) run TLS.
import { ConfigKey } from './backend-utils';

export const getBaseUrl = (configKey: ConfigKey, useTls = false): string => {
  const port = assignedPortForConfig(configKey);
  const protocol = useTls ? 'https' : 'http';
  return `${protocol}://127.0.0.1:${port}`;
};

/** Replaces environment-dependent values like URLs and config paths with placeholders for generic snapshots */
export const sanitizeEnvironmentDependents = async (page: Page) => {
  await page.evaluate(() => {
    // Recursively sanitize all text nodes in the DOM
    const fullUrlRegexes: RegExp[] = [
      /https?:\/\/127\.0\.0\.1:\d+/g,
      /https?:\/\/localhost:\d+/g,
      /http:\/\/127\.0\.0\.1:\d+/g,
      /http:\/\/localhost:\d+/g
    ];
    const domainRegexes: RegExp[] = [
      /127\.0\.0\.1:\d+/g,
      /localhost:\d+/g,
      /127\.0\.0\.1/g,
      /localhost/g
    ];
    /** Replace environment-dependent URLs and domains in a string.*/
    const sanitizeText = (text: string): string => {
      let sanitized = text;
      fullUrlRegexes.forEach((r: RegExp) => {
        sanitized = sanitized.replace(r, '<protocol://base_url>');
      });
      domainRegexes.forEach((r: RegExp) => {
        sanitized = sanitized.replace(r, '<base_url>');
      });
      return sanitized;
    };
    const isHTMLElement = (node: Node): node is HTMLElement => {
      return node.nodeType === Node.ELEMENT_NODE;
    }

    /** Recursively walk the DOM and sanitize all text nodes. */
    const walk = (node: Node): void => {
      if (node.nodeType === Node.TEXT_NODE) {
        node.textContent = sanitizeText(node.textContent || '');
      } else if (isHTMLElement(node)) {
        // Special handling for config location
        if (node.id === 'config-location') {
          node.textContent = '<coordinator_config_location>';
        }
        node.childNodes.forEach(walk);
      }
    };
    walk(document.body);
  });
}

export const expand_and_sanitize_host_install = async (
  page: Page,
  configKey: ConfigKey
) => {
  await page.goto(getBaseUrl(configKey) + '#hosts');
  // Open the collapsible by checking the toggle input
  // The checkbox input is hidden (CSS); click the visible header/label instead.
  await page.waitForSelector('#host-install-header');
  await page.click('#host-install-header');
  await page.waitForSelector('#host-install-content', { state: 'visible' });
  // Sanitize dynamic install command and config path for stable snapshots
  await sanitizeEnvironmentDependents(page);
}
