import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './tests',
  timeout: 60_000,
  use: { baseURL: 'http://localhost:8788' },
  // wrangler dev serves dist/ with the _headers file; astro preview does not.
  webServer: {
    command: 'bunx wrangler dev --port 8788',
    url: 'http://localhost:8788/playground/',
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
});
