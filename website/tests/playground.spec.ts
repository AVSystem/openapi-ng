import { expect, test } from '@playwright/test';

const INVALID_SPEC =
  'openapi: 3.0.3\ninfo: {title: x, version: "1"}\npaths: {/a: {get: {responses: {"200": {description: ok}}}}}';

test('generates the petstore client in the browser', async ({ page }) => {
  await page.goto('/playground/');
  expect(await page.evaluate(() => globalThis.crossOriginIsolated)).toBe(true);
  await expect(page.locator('#pg-status')).toContainText('runs in your browser');
  await expect(page.locator('#pg-tree li[data-path]')).toHaveCount(6);
  await page.locator('#pg-tree li[data-path="rest/pet.rest.generated.ts"]').click();
  await expect(page.locator('#pg-code')).toContainText('listPets');
});

test('shows a typed diagnostic for an unsupported spec', async ({ page }) => {
  await page.goto('/playground/');
  await expect(page.locator('#pg-status')).toContainText('runs in your browser');
  const editor = page.locator('#pg-editor .cm-content');
  await editor.click();
  await page.keyboard.press('ControlOrMeta+a');
  await page.keyboard.insertText(INVALID_SPEC);
  await expect(page.locator('#pg-diagnostics li.is-error')).toContainText('E_POLICY_VIOLATION');
  await expect(page.locator('#pg-diagnostics li.is-error')).toContainText('missing-operation-id');
});

test('share link round-trips the spec', async ({ page, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  await page.goto('/playground/');
  await expect(page.locator('#pg-status')).toContainText('runs in your browser');
  const editor = page.locator('#pg-editor .cm-content');
  await editor.click();
  await page.keyboard.press('ControlOrMeta+a');
  await page.keyboard.insertText('openapi: 3.0.3\ninfo: {title: shared, version: "1"}\npaths: {}');
  await page.locator('#pg-share').click();
  await expect(page).toHaveURL(/#spec=/);
  await page.reload();
  await expect(page.locator('#pg-editor .cm-content')).toContainText('title: shared');
});
