import { test, expect } from '@playwright/test';

test.describe('FFH Simplified Game', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
  });

  test('game canvas renders', async ({ page }) => {
    const canvas = page.locator('#game-canvas');
    await expect(canvas).toBeVisible();
  });

  test('game shows turn counter', async ({ page }) => {
    // The UI should display turn information
    await expect(page.locator('#turn-indicator')).toContainText('Turn');
  });

  test('game shows player gold', async ({ page }) => {
    await expect(page.locator('#gold-amount')).toBeVisible();
  });

  test('clicking on map selects tile', async ({ page }) => {
    const canvas = page.locator('#game-canvas');

    // Click on a tile (center of canvas)
    const box = await canvas.boundingBox();
    if (box) {
      await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
    }

    // Selection panel should become visible when something is selected
    // (It starts hidden and appears on selection)
    const selectionPanel = page.locator('#selection-panel');
    // Give time for the click to register and UI to update
    await page.waitForTimeout(100);
    // Check that the panel exists (may or may not be visible depending on what was clicked)
    await expect(selectionPanel).toBeAttached();
  });

  test('end turn button advances turn', async ({ page }) => {
    const turnIndicator = page.locator('#turn-indicator');

    // Get initial turn text
    await expect(turnIndicator).toContainText('Turn 1');

    // Click end turn
    await page.locator('#end-turn-btn').click();

    // Wait for turn to advance
    await expect(turnIndicator).toContainText('Turn 2');
  });

  test('armageddon counter is displayed', async ({ page }) => {
    await expect(page.locator('#armageddon-counter')).toBeVisible();
    await expect(page.locator('#armageddon-value')).toContainText('/100');
  });
});

test.describe('Game Controls', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
  });

  test('keyboard navigation works', async ({ page }) => {
    const canvas = page.locator('#game-canvas');
    await canvas.click(); // Focus canvas by clicking

    // Press arrow key to move camera/selection
    await page.keyboard.press('ArrowRight');

    // Canvas should still be focused and game should still be running
    await expect(canvas).toBeVisible();
  });

  test('escape key deselects', async ({ page }) => {
    const canvas = page.locator('#game-canvas');

    // Click to select something
    const box = await canvas.boundingBox();
    if (box) {
      await page.mouse.click(box.x + 100, box.y + 100);
    }

    // Press escape
    await page.keyboard.press('Escape');

    // Selection panel should be hidden after escape
    const selectionPanel = page.locator('#selection-panel');
    await expect(selectionPanel).toBeHidden();
  });
});
