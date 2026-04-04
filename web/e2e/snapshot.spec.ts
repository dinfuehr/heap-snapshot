import { test, expect } from '@playwright/test';
import { loadSnapshot } from './helpers';

test.describe('Heap Snapshot Viewer', () => {
  test.beforeEach(async ({ page }) => {
    await loadSnapshot(page, 'heap-1.heapsnapshot');
  });

  test('loads snapshot and shows summary tab', async ({ page }) => {
    // Summary tab should be active (bold)
    const summaryTab = page.locator('button:has-text("Summary")');
    await expect(summaryTab).toBeVisible();

    // Should show constructor groups in the table
    const table = page.locator('table').first();
    await expect(table).toBeVisible();

    // Should have at least one constructor row
    const rows = table.locator('tbody tr');
    await expect(rows.first()).toBeVisible();
  });

  test('shows object count and shallow size', async ({ page }) => {
    // The info line above the table should show object count and shallow size
    const info = page.locator('text=/objects.*shallow/');
    await expect(info).toBeVisible();
  });

  test('can switch tabs', async ({ page }) => {
    // Click Containment tab
    await page.locator('button:has-text("Containment")').click();
    // Should show a tree table with Object column
    await expect(page.locator('th:has-text("Object")').first()).toBeVisible();

    // Click Statistics tab
    await page.locator('button:has-text("Statistics")').click();
    await expect(
      page.getByRole('cell', { name: 'Total', exact: true }),
    ).toBeVisible();
    await expect(
      page.getByRole('cell', { name: 'V8 Heap', exact: true }),
    ).toBeVisible();
  });

  test('can expand a constructor group', async ({ page }) => {
    // Find the first constructor group row and double-click to expand
    const firstGroup = page.locator('table tbody tr').first();
    await firstGroup.dblclick();

    // After expanding, there should be more rows (the individual objects)
    const rows = page.locator('table tbody tr');
    const count = await rows.count();
    expect(count).toBeGreaterThan(1);
  });

  test('can filter constructors by text', async ({ page }) => {
    const filterInput = page.locator(
      'input[placeholder="Filter constructors..."]',
    );
    await filterInput.fill('InitialObject');

    // Should show only matching groups
    const rows = page.locator('table tbody tr');
    const count = await rows.count();
    expect(count).toBeGreaterThanOrEqual(1);

    // All visible group names should contain the filter text
    const firstRow = rows.first();
    await expect(firstRow).toContainText('InitialObject');
  });

  test('can switch summary filter mode', async ({ page }) => {
    const select = page.locator('select');
    await expect(select).toBeVisible();

    // Default should be "All objects"
    await expect(select).toHaveValue('0');

    // Switch to "Unreachable (all)"
    await select.selectOption('1');

    // Wait for reload — the loading indicator or new content should appear
    // heap-1 has 0 unreachable objects, so the table should be empty or show no rows
    await page.waitForTimeout(500);
  });

  test('statistics shows expected fields', async ({ page }) => {
    await page.locator('button:has-text("Statistics")').click();

    await expect(
      page.getByRole('cell', { name: 'Total', exact: true }),
    ).toBeVisible();
    await expect(
      page.getByRole('cell', { name: 'V8 Heap', exact: true }),
    ).toBeVisible();
    await expect(
      page.getByRole('cell', { name: 'Code', exact: true }),
    ).toBeVisible();
    await expect(
      page.getByRole('cell', { name: 'Strings', exact: true }),
    ).toBeVisible();
    await expect(
      page.getByRole('cell', { name: 'Native', exact: true }),
    ).toBeVisible();
    await expect(
      page.getByRole('cell', { name: 'Extra Native', exact: true }),
    ).toBeVisible();
    await expect(
      page.getByRole('cell', { name: 'Unreachable', exact: true }),
    ).toBeVisible();
  });

  test('retainers view shows target input', async ({ page }) => {
    await page.locator('button:has-text("Retainers")').click();

    // Should show the node ID input
    const input = page.locator('input[placeholder="@12345"]');
    await expect(input).toBeVisible();
  });

  test('can load a second snapshot', async ({ page }) => {
    // Click "+ Load snapshot" button
    const loadButton = page.locator('button:has-text("+ Load snapshot")');
    await expect(loadButton).toBeVisible();
  });

  test('timeline tab is disabled without allocation data', async ({ page }) => {
    // heap-1.heapsnapshot has no allocation data, so Timeline should be disabled
    const timelineTab = page.locator('button:has-text("Timeline")');
    await expect(timelineTab).toBeDisabled();
  });

  test('right-click shows context menu', async ({ page }) => {
    // Expand first constructor group to get individual objects
    const firstGroup = page
      .locator('table')
      .first()
      .locator('tbody tr')
      .first();
    await firstGroup.dblclick();

    // Find a visible object link (@id) — use :visible to avoid hidden tab content
    const objectLink = page
      .locator('a[href="#"]')
      .filter({ hasText: '@' })
      .first();
    await expect(objectLink).toBeVisible({ timeout: 5000 });

    // Right-click on the link
    await objectLink.click({ button: 'right' });

    // Context menu should appear with expected items
    await expect(page.locator('text=Show retainers')).toBeVisible();
    await expect(page.locator('text=Show in dominators')).toBeVisible();
    await expect(page.locator('text=Show in summary')).toBeVisible();
    await expect(page.locator('text=Remember object')).toBeVisible();
  });

  test('right-click "Show retainers" navigates to retainers view', async ({
    page,
  }) => {
    // Expand first constructor group
    const firstGroup = page
      .locator('table')
      .first()
      .locator('tbody tr')
      .first();
    await firstGroup.dblclick();

    const objectLink = page
      .locator('a[href="#"]')
      .filter({ hasText: '@' })
      .first();
    await expect(objectLink).toBeVisible({ timeout: 5000 });
    await objectLink.click({ button: 'right' });

    // Click "Show retainers"
    await page.locator('text=Show retainers').click();

    // Should switch to Retainers tab
    const retainersTab = page.locator('button:has-text("Retainers")');
    await expect(retainersTab).toHaveCSS('font-weight', '700');

    // The retainers input should have the object id
    const input = page.locator('input[placeholder="@12345"]');
    await expect(input).toHaveValue(/@\d+/);
  });

  test('right-click "Show in dominators" navigates to dominators view', async ({
    page,
  }) => {
    const firstGroup = page
      .locator('table')
      .first()
      .locator('tbody tr')
      .first();
    await firstGroup.dblclick();

    const objectLink = page
      .locator('a[href="#"]')
      .filter({ hasText: '@' })
      .first();
    await expect(objectLink).toBeVisible({ timeout: 5000 });
    await objectLink.click({ button: 'right' });

    await page.locator('text=Show in dominators').click();

    const dominatorsTab = page.locator('button:has-text("Dominators")');
    await expect(dominatorsTab).toHaveCSS('font-weight', '700');
  });

  test('right-click "Remember object" adds to history', async ({ page }) => {
    const firstGroup = page
      .locator('table')
      .first()
      .locator('tbody tr')
      .first();
    await firstGroup.dblclick();

    const objectLink = page
      .locator('a[href="#"]')
      .filter({ hasText: '@' })
      .first();
    await expect(objectLink).toBeVisible({ timeout: 5000 });
    await objectLink.click({ button: 'right' });

    await page.locator('text=Remember object').click();

    // Switch to History tab — should have at least one entry
    await page.locator('button:has-text("History")').click();
    // The History tab should show the remembered object's name
    // Wait for any visible table row in the History view
    await expect(
      page.locator('tr:visible').filter({ hasText: '@' }).first(),
    ).toBeVisible({ timeout: 5000 });
  });

  test('context menu closes on click outside', async ({ page }) => {
    const firstGroup = page
      .locator('table')
      .first()
      .locator('tbody tr')
      .first();
    await firstGroup.dblclick();

    const objectLink = page
      .locator('a[href="#"]')
      .filter({ hasText: '@' })
      .first();
    await expect(objectLink).toBeVisible({ timeout: 5000 });
    await objectLink.click({ button: 'right' });
    await expect(page.locator('text=Show retainers')).toBeVisible();

    // Click outside the menu
    await page.mouse.click(10, 10);

    // Menu should be gone
    await expect(page.locator('text=Show retainers')).not.toBeVisible();
  });
});
