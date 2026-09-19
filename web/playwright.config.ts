import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir: './tests',
  testMatch: '**/*.spec.ts',
  workers: 1,
  timeout: 60000,
  use: { baseURL: 'http://localhost:8765', headless: true },
  reporter: [['list'], ['html', { open: 'never' }]],
});
