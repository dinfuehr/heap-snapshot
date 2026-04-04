import { Page } from '@playwright/test';
import * as path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const TEST_DATA_DIR = path.resolve(__dirname, '../../tests/data');

/**
 * Load a heap snapshot file into the web UI.
 * Waits for the Summary tab to render (indicating successful load).
 */
export async function loadSnapshot(page: Page, filename: string) {
  await page.goto('/');

  const filePath = path.join(TEST_DATA_DIR, filename);

  // Set the file on the hidden file input
  const fileInput = page.locator('input[type="file"]');
  await fileInput.setInputFiles(filePath);

  // Wait for the snapshot to load — the tab bar should appear
  await page.locator('button:has-text("Summary")').waitFor({ timeout: 15000 });
}
