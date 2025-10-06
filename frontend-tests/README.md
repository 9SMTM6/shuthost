Playwright frontend tests for shuthost

Minimal install (Chromium only)

1) Install Node deps without downloading browsers:

   # optional: skip browser download
   export PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1
   npm install

2) Install only Chromium (fast, single browser):

   npm run install-chromium

Run tests

- Run all tests:

  npm test

- Run only visual tests, updating snapshots:

  npx playwright test tests/visual-regression.spec.ts --update-snapshots

CI notes

- On CI, prefer using the single-browser installer or a system Chrome image.
- Example (Linux):

  export PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1
  npm ci
  # ensure chromium / google-chrome is installed in image
  npm test

