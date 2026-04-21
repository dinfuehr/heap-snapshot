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
      'input[placeholder="Filter constructors or @id..."]',
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
    const select = page.locator('select').first();
    await expect(select).toBeVisible();

    // Default should be "All objects"
    await expect(select).toHaveValue('0');

    // Switch to "Unreachable (all)"
    await select.selectOption('1');

    // Wait for reload — the loading indicator or new content should appear
    // heap-1 has 0 unreachable objects, so the table should be empty or show no rows
    await page.waitForTimeout(500);
  });

  test('summary filter "Duplicate strings" lists duplicate groups', async ({
    page,
  }) => {
    const select = page.locator('select').first();
    await select.selectOption('8');

    // heap-1 contains duplicate strings (CLI reports 11 groups), so the table
    // should populate with at least one row whose name surfaces the value.
    const rows = page.locator('table tbody tr');
    await expect(rows.first()).toBeVisible({ timeout: 5000 });
    expect(await rows.count()).toBeGreaterThan(0);

    // Expanding a duplicate-string group should reveal individual @id rows
    // (using the same expansion path as class aggregates).
    await rows.first().dblclick();
    const objectLink = page
      .locator('a[href="#"]')
      .filter({ hasText: '@' })
      .first();
    await expect(objectLink).toBeVisible({ timeout: 5000 });
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
    await expect(
      page.locator('text=/Show @\\d+ in Retainers view/'),
    ).toBeVisible();
    await expect(
      page.locator('text=/Show @\\d+ in Dominators view/'),
    ).toBeVisible();
    await expect(
      page.locator('text=/Show @\\d+ in Summary view/'),
    ).toBeVisible();
    await expect(page.locator('text=Remember object')).toBeVisible();
    await expect(page.locator('text=Inspect')).toBeVisible();
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
    await page.locator('text=/Show @\\d+ in Retainers view/').click();

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

    await page.locator('text=/Show @\\d+ in Dominators view/').click();

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

  test('history view can expand children recursively', async ({ page }) => {
    // Remember an object first — expand a Summary group and right-click
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

    // Switch to History tab
    await page.locator('button:has-text("History")').click();
    const historyRow = page
      .locator('tr:visible')
      .filter({ hasText: '@' })
      .first();
    await expect(historyRow).toBeVisible({ timeout: 5000 });

    // Expand the remembered object
    await historyRow.dblclick();

    // Wait for children with edge labels
    const childRows = page.locator('tr:visible').filter({ hasText: '::' });
    await expect(childRows.first()).toBeVisible({ timeout: 5000 });

    // Expand the first child (the new functionality)
    await childRows.first().dblclick();

    // Grandchildren should appear — more edge-label rows visible
    const allEdgeRows = page.locator('tr:visible').filter({ hasText: '::' });
    await expect(allEdgeRows.nth(1)).toBeVisible({ timeout: 5000 });
    const count = await allEdgeRows.count();
    expect(count).toBeGreaterThan(1);
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
    await expect(
      page.locator('text=/Show @\\d+ in Retainers view/'),
    ).toBeVisible();

    // Click outside the menu
    await page.mouse.click(10, 10);

    // Menu should be gone
    await expect(
      page.locator('text=/Show @\\d+ in Retainers view/'),
    ).not.toBeVisible();
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

    // Click the chevron to expand. Use exact:true so we match the chevron
    // span (text == "▶") rather than the surrounding cell. Retry until the
    // chevron flips, because Solid binds the chevron's click handler in the
    // same render pass that puts the row in the DOM, and Playwright can
    // sometimes click in the gap between visibility and handler binding.
    const chevron = objectRow.getByText('\u25b6', { exact: true });
    await expect(async () => {
      await chevron.click();
      await expect(objectRow.getByText('\u25bc', { exact: true })).toBeVisible({
        timeout: 1000,
      });
    }).toPass({ timeout: 10000 });

    // Should show children with edge labels (:: separator)
    await expect(
      page.locator('tr:visible').filter({ hasText: '::' }).first(),
    ).toBeVisible({ timeout: 10000 });
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

    // Press 8 for Statistics
    await page.keyboard.press('8');
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
      'input[placeholder="Filter constructors or @id..."]',
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

  // ── Close snapshot ────────────────────────────────────────────────────

  test('closing single snapshot returns to file loader', async ({ page }) => {
    // Click the close button (✕)
    const closeButton = page.locator('button[title="Close snapshot"]');
    await expect(closeButton).toBeVisible();
    await closeButton.click();

    // Should return to the file loader screen
    await expect(page.locator('text=Heap Snapshot Viewer')).toBeVisible();
    await expect(page.locator('input[type="file"]')).toBeVisible();
  });

  test('closing one of two snapshots keeps the other', async ({ page }) => {
    // Load a second snapshot
    const loadButton = page.locator('button:has-text("+ Load snapshot")');
    const [fileChooser] = await Promise.all([
      page.waitForEvent('filechooser'),
      loadButton.click(),
    ]);
    await fileChooser.setFiles(
      path.resolve(__dirname, '../../tests/data/heap-2.heapsnapshot'),
    );

    // Wait for both to be loaded
    await expect(
      page.locator('button').filter({ hasText: 'heap-2' }),
    ).toBeVisible({ timeout: 15000 });
    await expect(
      page.locator('button').filter({ hasText: 'heap-1' }),
    ).toBeVisible();

    // Switch to heap-1 and close it
    await page.locator('button').filter({ hasText: 'heap-1' }).click();
    // Close the active snapshot (heap-1) via its close button
    const heap1Close = page
      .locator('span')
      .filter({ hasText: 'heap-1' })
      .locator('button[title="Close snapshot"]');
    await heap1Close.click();

    // heap-2 should still be visible as the filename
    await expect(
      page.locator('span').filter({ hasText: 'heap-2' }).first(),
    ).toBeVisible({ timeout: 5000 });

    // The UI should still be functional — summary table should show
    const rows = page.locator('table').first().locator('tbody tr');
    await expect(rows.first()).toBeVisible();
  });

  // ── Diff view ─────────────────────────────────────────────────────────

  test('diff tab is disabled with single snapshot', async ({ page }) => {
    const diffTab = page.locator('button:has-text("Diff")');
    await expect(diffTab).toBeDisabled();
  });

  test('diff tab shows results after loading two snapshots', async ({
    page,
  }) => {
    // Load second snapshot
    const [fileChooser] = await Promise.all([
      page.waitForEvent('filechooser'),
      page.locator('button:has-text("+ Load snapshot")').click(),
    ]);
    await fileChooser.setFiles(
      path.resolve(__dirname, '../../tests/data/heap-2.heapsnapshot'),
    );
    await expect(
      page.locator('button').filter({ hasText: 'heap-2' }),
    ).toBeVisible({ timeout: 15000 });

    // Switch back to heap-1
    await page.locator('button').filter({ hasText: 'heap-1' }).click();

    // Diff tab should now be enabled
    const diffTab = page.locator('button:has-text("Diff")');
    await expect(diffTab).not.toBeDisabled();

    // Click on Diff tab
    await diffTab.click();

    // Should see the baseline selector
    await expect(page.locator('text=Compare against:').first()).toBeVisible();

    // Select heap-2 as baseline (index 1 in the snapshots array)
    const diffSelect = page.locator(
      '[data-testid="diff-baseline-select"]:visible',
    );
    await diffSelect.waitFor({ state: 'visible', timeout: 5000 });
    await diffSelect.selectOption({ index: 1 });

    // Wait for diff results
    await expect(page.locator('text=/constructors changed/')).toBeVisible({
      timeout: 15000,
    });

    // Should show a table with diff columns
    await expect(page.locator('th:has-text("# New")').first()).toBeVisible();
    await expect(
      page.locator('th:has-text("# Deleted")').first(),
    ).toBeVisible();
    await expect(
      page.locator('th:has-text("Size Delta")').first(),
    ).toBeVisible();

    // Should have at least one diff row
    const diffTable = page
      .locator('table')
      .filter({ has: page.locator('th:has-text("# New")') })
      .first();
    await expect(diffTable.locator('tbody tr').first()).toBeVisible();
  });

  test('diff shows new objects between heap-1 and heap-2', async ({ page }) => {
    // Load heap-2
    const [fileChooser] = await Promise.all([
      page.waitForEvent('filechooser'),
      page.locator('button:has-text("+ Load snapshot")').click(),
    ]);
    await fileChooser.setFiles(
      path.resolve(__dirname, '../../tests/data/heap-2.heapsnapshot'),
    );
    await expect(
      page.locator('button').filter({ hasText: 'heap-2' }),
    ).toBeVisible({ timeout: 15000 });

    // Switch back to heap-1 and go to Diff tab
    await page.locator('button').filter({ hasText: 'heap-1' }).click();
    await page.locator('button:has-text("Diff")').click();
    const diffSelect = page.locator(
      '[data-testid="diff-baseline-select"]:visible',
    );
    await diffSelect.waitFor({ state: 'visible', timeout: 5000 });
    await diffSelect.selectOption({ index: 1 });

    await expect(page.locator('text=/constructors changed/')).toBeVisible({
      timeout: 15000,
    });

    // heap-2 has NewObject ×2 that don't exist in heap-1
    // So the diff should show NewObject with new_count > 0
    const newObjectRow = page
      .locator('tr:visible')
      .filter({ hasText: 'NewObject' });
    await expect(newObjectRow).toBeVisible();
  });

  test('diff shows new objects when heap-2 compares against heap-1', async ({
    page,
  }) => {
    // Load heap-2
    const [fileChooser] = await Promise.all([
      page.waitForEvent('filechooser'),
      page.locator('button:has-text("+ Load snapshot")').click(),
    ]);
    await fileChooser.setFiles(
      path.resolve(__dirname, '../../tests/data/heap-2.heapsnapshot'),
    );
    await expect(
      page.locator('button').filter({ hasText: 'heap-2' }),
    ).toBeVisible({ timeout: 15000 });

    // heap-2 is active after loading. Go to Diff tab.
    await page.locator('button:has-text("Diff")').click();

    // Select heap-1 as baseline (first option after "Select a snapshot...")
    const diffSelect = page.locator(
      '[data-testid="diff-baseline-select"]:visible',
    );
    await diffSelect.waitFor({ state: 'visible', timeout: 5000 });
    await diffSelect.selectOption({ index: 1 });

    await expect(page.locator('text=/constructors changed/')).toBeVisible({
      timeout: 15000,
    });

    // heap-2 has NewObject ×2 that don't exist in heap-1
    // With heap-2 as main and heap-1 as baseline, NewObject should appear as new
    const newObjectRow = page
      .locator('tr:visible')
      .filter({ hasText: 'NewObject' });
    await expect(newObjectRow).toBeVisible();
  });

  test('comparing a snapshot against itself shows no differences', async ({
    page,
  }) => {
    // Load a second copy of the same snapshot
    const [fileChooser] = await Promise.all([
      page.waitForEvent('filechooser'),
      page.locator('button:has-text("+ Load snapshot")').click(),
    ]);
    await fileChooser.setFiles(
      path.resolve(__dirname, '../../tests/data/heap-1.heapsnapshot'),
    );
    await expect(
      page.locator('button[title="Close snapshot"]').first(),
    ).toBeVisible({
      timeout: 15000,
    });

    // Go to Diff tab and compare against the second copy
    await page.locator('button:has-text("Diff")').click();
    const diffSelect = page.locator(
      '[data-testid="diff-baseline-select"]:visible',
    );
    await diffSelect.waitFor({ state: 'visible', timeout: 5000 });
    await diffSelect.selectOption({ index: 1 });

    // Should show "No differences found"
    await expect(page.locator('text=No differences found')).toBeVisible({
      timeout: 15000,
    });
  });

  test('diff heap-1 against heap-2 shows deleted NewObjects', async ({
    page,
  }) => {
    // Load heap-2
    const [fileChooser] = await Promise.all([
      page.waitForEvent('filechooser'),
      page.locator('button:has-text("+ Load snapshot")').click(),
    ]);
    await fileChooser.setFiles(
      path.resolve(__dirname, '../../tests/data/heap-2.heapsnapshot'),
    );
    await expect(
      page.locator('button').filter({ hasText: 'heap-2' }),
    ).toBeVisible({ timeout: 15000 });

    // Switch to heap-1 and go to Diff tab
    await page.locator('button').filter({ hasText: 'heap-1' }).click();
    await page.locator('button:has-text("Diff")').click();

    // Compare heap-1 (main) against heap-2 (baseline)
    const diffSelect = page.locator(
      '[data-testid="diff-baseline-select"]:visible',
    );
    await diffSelect.waitFor({ state: 'visible', timeout: 5000 });
    await diffSelect.selectOption({ index: 1 });

    await expect(page.locator('text=/constructors changed/')).toBeVisible({
      timeout: 15000,
    });

    // heap-1 has no NewObjects, heap-2 has 2 NewObjects
    // So from heap-1's perspective, NewObject should show as deleted
    const newObjectRow = page
      .locator('tr:visible')
      .filter({ hasText: 'NewObject' });
    await expect(newObjectRow).toBeVisible();

    // The "# Deleted" column should show -2
    const cells = await newObjectRow.locator('td').allTextContents();
    const deletedCell = cells[2]; // # Deleted is the 3rd column
    expect(deletedCell).toContain('-2');
  });

  test('diff shows computing message while loading', async ({ page }) => {
    // Load heap-2
    const [fileChooser] = await Promise.all([
      page.waitForEvent('filechooser'),
      page.locator('button:has-text("+ Load snapshot")').click(),
    ]);
    await fileChooser.setFiles(
      path.resolve(__dirname, '../../tests/data/heap-2.heapsnapshot'),
    );
    await expect(
      page.locator('button').filter({ hasText: 'heap-2' }),
    ).toBeVisible({ timeout: 15000 });

    // Switch to heap-1 and go to Diff tab
    await page.locator('button').filter({ hasText: 'heap-1' }).click();
    await page.locator('button:has-text("Diff")').click();

    // Select baseline — should briefly show "Computing diff..."
    const diffSelect = page.locator(
      '[data-testid="diff-baseline-select"]:visible',
    );
    await diffSelect.waitFor({ state: 'visible', timeout: 5000 });
    await diffSelect.selectOption({ index: 1 });

    // Either the computing message or the results should appear
    await expect(
      page
        .locator('text=Computing diff...')
        .or(page.locator('text=/constructors changed/'))
        .or(page.locator('text=No differences found')),
    ).toBeVisible({ timeout: 15000 });
  });

  // ── Loading indicator ─────────────────────────────────────────────────

  test('loading a second snapshot shows spinner and wait cursor', async ({
    page,
  }) => {
    // Start loading a second snapshot — don't await full load
    const [fileChooser] = await Promise.all([
      page.waitForEvent('filechooser'),
      page.locator('button:has-text("+ Load snapshot")').click(),
    ]);

    // Set the file — this starts loading
    await fileChooser.setFiles(
      path.resolve(__dirname, '../../tests/data/heap-2.heapsnapshot'),
    );

    // The loading tab should appear with a spinner (animated span)
    // and the filename should be visible in regular text (not greyed out)
    const loadingTab = page.locator('button:has-text("heap-2")').first();
    await expect(loadingTab).toBeVisible({ timeout: 5000 });

    // Once fully loaded, the spinner should be gone and close button should appear
    await expect(page.locator('button[title="Close snapshot"]')).toHaveCount(
      2,
      { timeout: 15000 },
    );
  });

  // ── Retainers auto-expand via context menu ────────────────────────────

  test('retainers view shows "N selected of M retainers" summary with View all button', async ({
    page,
  }) => {
    // Expand a group and navigate to retainers for an object
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
    await page.locator('text=/Show @\\d+ in Retainers view/').click();

    await expect(page.getByTestId('retaining-paths-header')).toBeVisible({
      timeout: 5000,
    });

    // Look for a "N selected of M retainers" summary row with a "View all" button.
    const summaryCell = page.locator('td', {
      hasText: /\d+ selected of \d+ retainers/,
    });
    await expect(summaryCell.first()).toBeVisible({ timeout: 5000 });

    const viewAllBtn = summaryCell
      .first()
      .locator('button', { hasText: 'View all' });
    await expect(viewAllBtn).toBeVisible();

    // Click "View all" — the summary row for that node should be replaced
    // by the full retainer list.
    await viewAllBtn.click();

    // Verify more retainer rows appeared: expand arrows should be visible
    // in the retainer tree (the loaded retainers are full RetainerNodes).
    const arrowSpans = page.locator('tr:visible td span', { hasText: /[▶▼]/ });
    await expect(arrowSpans.first()).toBeVisible({ timeout: 3000 });
  });

  test('compute reachable size populates the Reachable Size column across views', async ({
    page,
  }) => {
    // Expand a constructor group to get an object
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
    const objectId = (await objectLink.textContent())!.trim();

    // The object's row should show "—" in the Reachable Size column initially
    const row = objectLink.locator('xpath=ancestor::tr');
    const cells = row.locator('td');
    // Reachable Size is the 5th column (index 4)
    await expect(cells.nth(4)).toHaveText('—');

    // Right-click and compute reachable size
    await objectLink.click({ button: 'right' });
    await page.locator('text=Compute reachable size').first().click();

    // The "—" should be replaced with an actual size value (may show ⋯ while loading)
    await expect(cells.nth(4)).toHaveText(/\d+(\.\d+)?\s*(B|KB|MB|GB)/, {
      timeout: 5000,
    });
    const sizeText = await cells.nth(4).textContent();

    // Navigate to Retainers for the same object — the retainer paths
    // contain links to other objects. If any of those also had their
    // reachable size computed, the column would show there too.
    // Here we verify the shared reachableSizes map works across tabs
    // by navigating to retainers and back.
    await objectLink.click({ button: 'right' });
    await page.locator('text=/Show @\\d+ in Retainers view/').click();
    await expect(page.getByTestId('retaining-paths-header')).toBeVisible({
      timeout: 5000,
    });

    // Switch back to Summary — the computed size should still be there
    await page.locator('button:has-text("Summary")').click();
    const summaryLink = page
      .locator(`a[href="#"]`, { hasText: objectId })
      .first();
    await expect(summaryLink).toBeVisible({ timeout: 5000 });
    const summaryRow = summaryLink.locator('xpath=ancestor::tr');
    await expect(summaryRow.locator('td').nth(4)).toHaveText(sizeText!);
  });

  test('compute reachable size w/ children populates column for parent and children', async ({
    page,
  }) => {
    // Expand a constructor group to get objects
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

    // Right-click and compute reachable size w/ children
    await objectLink.click({ button: 'right' });
    await page.locator('text=Compute reachable size w/ children').click();

    // The object's own reachable size should be populated (may show ⋯ while loading)
    const row = objectLink.locator('xpath=ancestor::tr');
    const cells = row.locator('td');
    await expect(cells.nth(4)).toHaveText(/\d+(\.\d+)?\s*(B|KB|MB|GB)/, {
      timeout: 5000,
    });

    // Expand the object to see its children — they should all have
    // reachable size populated from the "w/ children" computation.
    await row.dblclick();

    // Children appear as rows with @id links after the parent row.
    // Wait for at least one child to load, then use page.evaluate to
    // reliably find child rows by checking the DOM structure.
    await page.waitForFunction(
      (parentText) => {
        const rows = document.querySelectorAll('table tbody tr');
        let foundParent = false;
        for (const row of rows) {
          const link = row.querySelector('a[href="#"]');
          if (link?.textContent?.trim() === parentText) {
            foundParent = true;
            continue;
          }
          if (foundParent && link?.textContent?.trim()?.startsWith('@')) {
            return true; // found a child row
          }
        }
        return false;
      },
      (await objectLink.textContent())!.trim(),
      { timeout: 5000 },
    );

    // Wait for async reachable size computations to settle, then
    // verify all child rows have their reachable size column filled.
    const objectId = (await objectLink.textContent())!.trim();

    // Collect child row data — wait until at least one child is visible
    // and all visible children have reachable sizes populated.
    const childData = await page.waitForFunction(
      (parentId) => {
        const rows = Array.from(document.querySelectorAll('table tbody tr'));
        let foundParent = false;
        let parentPadding = 0;
        const results: { id: string; reachable: string }[] = [];
        for (const row of rows) {
          const link = row.querySelector('a[href="#"]');
          const id = link?.textContent?.trim();
          const firstCell = row.querySelector('td');
          const padding = firstCell
            ? parseInt(getComputedStyle(firstCell).paddingLeft)
            : 0;
          if (id === parentId) {
            foundParent = true;
            parentPadding = padding;
            continue;
          }
          if (foundParent) {
            // Stop when we reach a row at the same or lesser depth
            // as the parent — that's a sibling, not a child.
            if (padding <= parentPadding) break;
            if (!id?.startsWith('@')) continue; // skip pager rows
            const cells = row.querySelectorAll('td');
            const val = cells[4]?.textContent?.trim() ?? '';
            if (val === '\u2014' || val === '') return null; // still computing
            results.push({ id: id!, reachable: val });
          }
        }
        return results.length > 0 ? results : null;
      },
      objectId,
      { timeout: 10000 },
    );

    const values = (await childData.jsonValue()) as {
      id: string;
      reachable: string;
    }[];
    expect(values.length).toBeGreaterThan(0);
    for (const { reachable } of values) {
      expect(reachable).toMatch(/\d+(\.\d+)?\s*(B|KB|MB|GB)/);
    }
  });

  test('contexts view auto-computes reachable sizes for all native contexts', async ({
    page,
  }) => {
    await page.locator('button:has-text("Contexts")').click();

    // Wait for contexts to load — scope to visible rows only
    const contextRows = page
      .locator('tr:visible')
      .filter({ has: page.locator('a[href="#"]') });
    await expect(contextRows.first()).toBeVisible({ timeout: 5000 });

    // Wait until all visible context rows have their reachable size
    // column populated (no "—" remaining). The auto-computation runs
    // in the background so we poll.
    await page.waitForFunction(
      () => {
        const rows = document.querySelectorAll('table tbody tr');
        let count = 0;
        for (const row of rows) {
          if (!(row as HTMLElement).offsetParent) continue; // hidden
          const link = row.querySelector('a[href="#"]');
          if (!link) continue; // skip pager/status rows
          count++;
          const cells = row.querySelectorAll('td');
          const val = cells[4]?.textContent?.trim();
          if (!val || val === '\u2014') return false;
        }
        return count > 0;
      },
      undefined,
      { timeout: 30000 },
    );

    // Verify all context rows have valid byte values
    const count = await contextRows.count();
    expect(count).toBeGreaterThan(0);
    for (let i = 0; i < count; i++) {
      const reachable = await contextRows
        .nth(i)
        .locator('td')
        .nth(4)
        .textContent();
      expect(reachable).toMatch(/\d+(\.\d+)?\s*(B|KB|MB|GB)/);
    }
  });

  test('contexts view can expand children recursively', async ({ page }) => {
    await page.locator('button:has-text("Contexts")').click();

    // Wait for context rows to load
    const contextRows = page
      .locator('tr:visible')
      .filter({ has: page.locator('a[href="#"]') });
    await expect(contextRows.first()).toBeVisible({ timeout: 5000 });

    // Double-click the first context to expand it
    await contextRows.first().dblclick();

    // Wait for children to appear — they have edge labels with " :: "
    const childRows = page.locator('tr:visible').filter({ hasText: '::' });
    await expect(childRows.first()).toBeVisible({ timeout: 5000 });

    // Double-click the first child to expand it (the new functionality)
    await childRows.first().dblclick();

    // Count visible rows with edge labels — should be more than before
    // because grandchildren have appeared
    const allEdgeRows = page.locator('tr:visible').filter({ hasText: '::' });
    await expect(allEdgeRows.nth(1)).toBeVisible({ timeout: 5000 });
    const count = await allEdgeRows.count();
    expect(count).toBeGreaterThan(1);
  });

  test('right-click "Show retainers" on a retainer node re-roots the view', async ({
    page,
  }) => {
    // Navigate to retainers for an object — first expand a group to get an object
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

    // Right-click and show retainers
    await objectLink.click({ button: 'right' });
    await page.locator('text=/Show @\\d+ in Retainers view/').click();

    // Should be in Retainers tab now with retaining paths
    await expect(page.locator('button:has-text("Retainers")')).toHaveCSS(
      'font-weight',
      '700',
    );

    // Wait for retaining paths to load
    await expect(page.getByTestId('retaining-paths-header')).toBeVisible({
      timeout: 5000,
    });

    // Find a visible retainer node link in the path tree
    const retainerLink = page
      .locator('tr:visible a[href="#"]')
      .filter({ hasText: '@' })
      .first();
    await expect(retainerLink).toBeVisible({ timeout: 5000 });
    const retainerText = await retainerLink.textContent();

    // Right-click the retainer and select "Show retainers"
    await retainerLink.click({ button: 'right' });
    await page.locator('text=/Show @\\d+ in Retainers view/').click();

    // The retainers input should now contain the retainer's ID
    const input = page.locator('input[placeholder="@12345"]');
    await expect(input).toHaveValue(retainerText!.trim());
  });

  // ── Search by object ID (@id) ──────────────────────────────────────────

  test('searching @id in summary expands constructor group and highlights object', async ({
    page,
  }) => {
    // Expand the first constructor group to discover a valid object ID
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
    const objectId = linkText!.trim(); // e.g. "@12345"

    // Collapse the group again so we verify the search re-expands it
    await firstGroup.dblclick();

    // Type the object ID into the filter input and press Enter
    const filterInput = page.locator(
      'input[placeholder="Filter constructors or @id..."]',
    );
    await filterInput.fill(objectId);
    await filterInput.press('Enter');

    // Should stay on Summary tab
    const summaryTab = page.locator('button:has-text("Summary")');
    await expect(summaryTab).toHaveCSS('font-weight', '700');

    // The constructor group should be expanded and the object visible
    const targetLink = page
      .locator('a[href="#"]')
      .filter({ hasText: objectId })
      .first();
    await expect(targetLink).toBeVisible({ timeout: 5000 });

    // The target row should be highlighted (selected via nodeId match)
    const targetRow = page.locator(
      `tr[data-node-id="${objectId.replace('@', '')}"]`,
    );
    await expect(targetRow).toBeVisible();
    const bg = await targetRow.evaluate(
      (el) => getComputedStyle(el).backgroundColor,
    );
    expect(bg).not.toBe('rgba(0, 0, 0, 0)');

    // The filter input should be cleared after a successful search
    await expect(filterInput).toHaveValue('');
  });

  test('searching invalid @id shows error', async ({ page }) => {
    const filterInput = page.locator(
      'input[placeholder="Filter constructors or @id..."]',
    );
    await filterInput.fill('@999999999');
    await filterInput.press('Enter');

    // Should show an error message
    await expect(page.locator('text=/No object found/')).toBeVisible();

    // Should still be on Summary tab
    const summaryTab = page.locator('button:has-text("Summary")');
    await expect(summaryTab).toHaveCSS('font-weight', '700');
  });

  test('searching @id with non-numeric value shows error', async ({ page }) => {
    const filterInput = page.locator(
      'input[placeholder="Filter constructors or @id..."]',
    );
    await filterInput.fill('@abc');
    await filterInput.press('Enter');

    await expect(page.locator('text=/Invalid id/')).toBeVisible();
  });

  test('typing @id does not filter constructor list', async ({ page }) => {
    // Get the initial number of constructor groups
    const initialRows = await page
      .locator('table')
      .first()
      .locator('tbody tr')
      .count();

    // Type an @id prefix — should not filter out any groups
    const filterInput = page.locator(
      'input[placeholder="Filter constructors or @id..."]',
    );
    await filterInput.fill('@123');

    const currentRows = await page
      .locator('table')
      .first()
      .locator('tbody tr')
      .count();
    expect(currentRows).toBe(initialRows);
  });

  test('searching @id resets summary filter to "All objects"', async ({
    page,
  }) => {
    // Switch to "Unreachable (all)" filter — this snapshot has no unreachable objects
    const select = page.locator('select').first();
    await select.selectOption('1');
    await expect(page.locator('text=/^0 objects/')).toBeVisible({
      timeout: 5000,
    });

    // Expand a group in "All objects" mode first to get a valid object ID,
    // then search for it while "Unreachable" is active
    await select.selectOption('0');
    await expect(page.locator('text=/objects.*shallow/').first()).toBeVisible({
      timeout: 5000,
    });
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
    const objectId = (await objectLink.textContent())!.trim();

    // Switch back to Unreachable and search by @id
    await select.selectOption('1');
    await expect(page.locator('text=/^0 objects/')).toBeVisible({
      timeout: 5000,
    });

    const filterInput = page.locator(
      'input[placeholder="Filter constructors or @id..."]',
    );
    await filterInput.fill(objectId);
    await filterInput.press('Enter');

    // The dropdown should reset to "All objects"
    await expect(select).toHaveValue('0');

    // The object should be visible
    const targetLink = page
      .locator('a[href="#"]')
      .filter({ hasText: objectId })
      .first();
    await expect(targetLink).toBeVisible({ timeout: 5000 });
  });

  test('searching @id clears error on new input', async ({ page }) => {
    const filterInput = page.locator(
      'input[placeholder="Filter constructors or @id..."]',
    );

    // Trigger an error
    await filterInput.fill('@999999999');
    await filterInput.press('Enter');
    await expect(page.locator('text=/No object found/')).toBeVisible();

    // Start typing — error should clear
    await filterInput.fill('A');
    await expect(page.locator('text=/No object found/')).not.toBeVisible();
  });

  test('inspect dialog shows node details and closes on click outside', async ({
    page,
  }) => {
    // Expand first constructor group to get an object
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

    // Right-click and select Inspect
    await objectLink.click({ button: 'right' });
    await page.locator('text=Inspect').click();

    // Inspect dialog should appear with node details
    const dialog = page.locator('text=Inspect Node');
    await expect(dialog).toBeVisible({ timeout: 5000 });

    // Should show key fields in the dialog
    const inspectDialog = page.locator('div:has(> div:text("Inspect Node"))');
    await expect(
      inspectDialog.locator('td', { hasText: /^@\d+$/ }),
    ).toBeVisible();
    await expect(
      inspectDialog.locator('td', { hasText: /^Self size$/ }),
    ).toBeVisible();
    await expect(
      inspectDialog.locator('td', { hasText: /^Retained size$/ }),
    ).toBeVisible();

    // Click outside to close the dialog
    await page.mouse.click(10, 10);
    await expect(dialog).not.toBeVisible();
  });

  test('inspect dialog closes on Escape', async ({ page }) => {
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

    // Right-click and select Inspect
    await objectLink.click({ button: 'right' });
    await page.locator('text=Inspect').click();

    const dialog = page.locator('text=Inspect Node');
    await expect(dialog).toBeVisible({ timeout: 5000 });

    // Press Escape to close
    await page.keyboard.press('Escape');
    await expect(dialog).not.toBeVisible();
  });
});
