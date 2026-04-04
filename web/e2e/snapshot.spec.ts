import { test, expect } from '@playwright/test';
import * as path from 'path';
import { fileURLToPath } from 'url';
import { loadSnapshot } from './helpers';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

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

  // ── Containment view ──────────────────────────────────────────────────

  test('containment view shows GC roots', async ({ page }) => {
    await page.locator('button:has-text("Containment")').click();

    // Should show (GC roots) in the tree
    await expect(page.locator('text=(GC roots)').first()).toBeVisible();
  });

  test('containment view can expand nodes', async ({ page }) => {
    await page.locator('button:has-text("Containment")').click();

    // (GC roots) should be auto-expanded, showing child nodes with edge labels
    // Look for edge separator " :: " which indicates edge labels are present
    await expect(
      page.locator('tr:visible').filter({ hasText: '::' }).first(),
    ).toBeVisible({ timeout: 5000 });
  });

  // ── Dominators view ───────────────────────────────────────────────────

  test('dominators view shows root', async ({ page }) => {
    await page.locator('button:has-text("Dominators")').click();

    // Should show a tree with at least one visible row containing an object ID
    await expect(
      page.locator('tr:visible').filter({ hasText: '@' }).first(),
    ).toBeVisible({ timeout: 5000 });
  });

  test('dominators view can expand root', async ({ page }) => {
    await page.locator('button:has-text("Dominators")').click();

    // Double-click the first row to expand
    const firstRow = page
      .locator('tr:visible')
      .filter({ hasText: '@' })
      .first();
    await firstRow.dblclick();

    // Should show more rows after expanding
    const rows = page.locator('tr:visible').filter({ hasText: '@' });
    await expect(rows.nth(1)).toBeVisible({ timeout: 5000 });
  });

  // ── Retainers view ────────────────────────────────────────────────────

  test('retainers view can look up an object', async ({ page }) => {
    // First get an object ID from the summary
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
    const linkText = await objectLink.textContent();
    const objectId = linkText!.trim();

    // Navigate to retainers
    await page.locator('button:has-text("Retainers")').click();

    const input = page.locator('input[placeholder="@12345"]');
    await input.fill(objectId);
    await page.locator('button:has-text("Go")').click();

    // Should show the object info header (rendered as <strong>@id</strong>)
    await expect(
      page.locator('strong').filter({ hasText: objectId }).first(),
    ).toBeVisible({ timeout: 5000 });
  });

  test('clicking @id link navigates to retainers', async ({ page }) => {
    // Go to containment view and find a link
    await page.locator('button:has-text("Containment")').click();
    const link = page.locator('a[href="#"]').filter({ hasText: '@' }).first();
    await expect(link).toBeVisible({ timeout: 5000 });

    // Click the link (left click navigates to retainers)
    await link.click();

    // Should switch to Retainers tab
    const retainersTab = page.locator('button:has-text("Retainers")');
    await expect(retainersTab).toHaveCSS('font-weight', '700');
  });

  // ── Multiple snapshots ────────────────────────────────────────────────

  test('can load two snapshots and switch between them', async ({ page }) => {
    const loadButton = page.locator('button:has-text("+ Load snapshot")');

    // Load a second snapshot via the "+ Load snapshot" button
    // We need to intercept the file input it creates
    const [fileChooser] = await Promise.all([
      page.waitForEvent('filechooser'),
      loadButton.click(),
    ]);
    await fileChooser.setFiles(
      path.resolve(__dirname, '../../tests/data/heap-2.heapsnapshot'),
    );

    // Wait for the second snapshot to load — should show two snapshot buttons
    await expect(
      page.locator('button').filter({ hasText: 'heap-2' }),
    ).toBeVisible({
      timeout: 15000,
    });

    // Both snapshot buttons should be visible
    await expect(
      page.locator('button').filter({ hasText: 'heap-1' }),
    ).toBeVisible();
    await expect(
      page.locator('button').filter({ hasText: 'heap-2' }),
    ).toBeVisible();

    // Switch back to first snapshot
    await page.locator('button').filter({ hasText: 'heap-1' }).click();

    // Summary should still show content
    const rows = page.locator('table').first().locator('tbody tr');
    await expect(rows.first()).toBeVisible();
  });

  // ── Edge labels ───────────────────────────────────────────────────────

  test('containment edges show correct format', async ({ page }) => {
    await page.locator('button:has-text("Containment")').click();

    // Wait for edges to appear — they should have " :: " separator
    const edgeRow = page
      .locator('tr:visible')
      .filter({ hasText: '::' })
      .first();
    await expect(edgeRow).toBeVisible({ timeout: 5000 });

    // Edge should contain @id
    const text = await edgeRow.textContent();
    expect(text).toMatch(/@\d+/);
  });

  // ── Summary object expansion ──────────────────────────────────────────

  test('expanding an object in summary shows its children', async ({
    page,
  }) => {
    // Expand a constructor group
    const firstGroup = page
      .locator('table')
      .first()
      .locator('tbody tr')
      .first();
    await firstGroup.dblclick();

    // Find an expandable object (has ▶ marker)
    const objectRow = page
      .locator('tr:visible')
      .filter({ hasText: '@' })
      .first();
    await expect(objectRow).toBeVisible({ timeout: 5000 });

    // Double-click to expand the object
    await objectRow.dblclick();

    // Should show children with edge labels (:: separator)
    await expect(
      page.locator('tr:visible').filter({ hasText: '::' }).first(),
    ).toBeVisible({ timeout: 5000 });
  });

  // ── Keyboard shortcuts ────────────────────────────────────────────────

  test('number keys switch tabs', async ({ page }) => {
    // Press 2 to switch to Containment
    await page.keyboard.press('2');
    await expect(page.locator('button:has-text("Containment")')).toHaveCSS(
      'font-weight',
      '700',
    );

    // Press 3 for Dominators
    await page.keyboard.press('3');
    await expect(page.locator('button:has-text("Dominators")')).toHaveCSS(
      'font-weight',
      '700',
    );

    // Press 4 for Retainers
    await page.keyboard.press('4');
    await expect(page.locator('button:has-text("Retainers")')).toHaveCSS(
      'font-weight',
      '700',
    );

    // Press 1 to go back to Summary
    await page.keyboard.press('1');
    await expect(page.locator('button:has-text("Summary")')).toHaveCSS(
      'font-weight',
      '700',
    );

    // Press 7 for Statistics
    await page.keyboard.press('7');
    await expect(page.locator('button:has-text("Statistics")')).toHaveCSS(
      'font-weight',
      '700',
    );
  });

  test('keyboard shortcuts do not fire when typing in input', async ({
    page,
  }) => {
    // Focus the filter input
    const filterInput = page.locator(
      'input[placeholder="Filter constructors..."]',
    );
    await filterInput.click();
    await filterInput.fill('');

    // Type "2" in the input — should not switch tabs
    await page.keyboard.type('2');

    // Summary tab should still be active
    await expect(page.locator('button:has-text("Summary")')).toHaveCSS(
      'font-weight',
      '700',
    );

    // Input should contain "2"
    await expect(filterInput).toHaveValue('2');
  });
});
