import { expect, test, type Page } from '@playwright/test';

const INVALID_SPEC =
  'openapi: 3.0.3\ninfo: {title: x, version: "1"}\npaths: {/a: {get: {responses: {"200": {description: ok}}}}}';

const COOKIE_PARAM_SPEC = [
  'openapi: 3.0.3',
  'info: {title: cookies, version: "1"}',
  'paths:',
  '  /a:',
  '    get:',
  '      operationId: getA',
  '      parameters: [{name: sid, in: cookie, schema: {type: string}}]',
  '      responses: {"200": {description: ok}}',
].join('\n');

const SNAKE_CASE_CONFIG = JSON.stringify({
  input: './openapi/internal.json',
  output: './libs/openapi-ng',
  emit: ['models', 'angular'],
  naming: { methodName: { from: '{operationId}', parse: '/^(?<all>.+)$/', format: '{capture.all}', case: 'snake' } },
});

async function ready(page: Page): Promise<void> {
  await page.goto('/playground/');
  await expect(page.locator('#pg-status')).toContainText('runs in your browser');
}

async function replaceText(page: Page, selector: string, text: string): Promise<void> {
  await page.locator(`${selector} .cm-content`).click();
  await page.keyboard.press('ControlOrMeta+a');
  await page.keyboard.insertText(text);
}

test('generates the petstore client in the browser', async ({ page }) => {
  await ready(page);
  expect(await page.evaluate(() => globalThis.crossOriginIsolated)).toBe(true);
  await expect(page.locator('#pg-tree li[data-path]')).toHaveCount(6);
  const petFile = page.locator('#pg-tree li[data-path="rest/pet.rest.generated.ts"] button');
  await expect(petFile).toHaveAttribute('title', 'rest/pet.rest.generated.ts');
  await petFile.click();
  await expect(page.locator('#pg-code')).toContainText('listPets');
});

test('shows a typed diagnostic for an unsupported spec', async ({ page }) => {
  await ready(page);
  await replaceText(page, '#pg-editor', INVALID_SPEC);
  await expect(page.locator('#pg-diagnostics li.is-error')).toContainText('E_POLICY_VIOLATION');
  await expect(page.locator('#pg-diagnostics li.is-error')).toContainText('missing-operation-id');
});

test('separates warnings from errors and notes by colour', async ({ page }) => {
  await ready(page);
  await replaceText(page, '#pg-editor', COOKIE_PARAM_SPEC);
  const warning = page.locator('#pg-diagnostics li.is-warning');
  await expect(warning).toContainText('unsupported-parameter-location');
  await replaceText(page, '#pg-config', '{"emit": ["models"], "input": "./spec.yaml"}');
  const note = page.locator('#pg-diagnostics li.is-note');
  await expect(note).toContainText('input and output are ignored');
  await replaceText(page, '#pg-config', '{"emit": [');
  const error = page.locator('#pg-diagnostics li.is-error');
  await expect(error).toContainText('not valid JSON');
  const colours = await page.evaluate(() => {
    const colourOf = (cls: string) => {
      const el = document.createElement('li');
      el.className = cls;
      document.querySelector('#pg-diagnostics')!.append(el);
      const colour = getComputedStyle(el).color;
      el.remove();
      return colour;
    };
    return ['is-warning', 'is-error', 'is-note', ''].map(colourOf);
  });
  expect(new Set(colours).size).toBe(4);
});

test('keeps diagnostic colours readable in both themes', async ({ page }) => {
  await ready(page);
  const contrasts = await page.evaluate(() => {
    const luminance = (colour: string) => {
      const [r, g, b] = colour.match(/\d+/g)!.map(Number).map(v => {
        const channel = v / 255;
        return channel <= 0.03928 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4;
      });
      return 0.2126 * r + 0.7152 * g + 0.0722 * b;
    };
    const ratio = (a: string, b: string) => {
      const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
      return (hi + 0.05) / (lo + 0.05);
    };
    const consoleEl = document.querySelector('#pg-console')!;
    const out: number[] = [];
    for (const theme of ['light', 'dark']) {
      document.documentElement.dataset.theme = theme;
      const background = getComputedStyle(consoleEl).backgroundColor;
      for (const className of ['is-warning', 'is-error', 'is-note']) {
        const li = document.createElement('li');
        li.className = className;
        document.querySelector('#pg-diagnostics')!.append(li);
        out.push(ratio(getComputedStyle(li).color, background));
        li.remove();
      }
    }
    return out;
  });
  for (const contrast of contrasts) expect(contrast).toBeGreaterThanOrEqual(4.5);
});

test('applies a pasted project config to the generated names', async ({ page }) => {
  await ready(page);
  await page.locator('#pg-tree li[data-path="rest/pet.rest.generated.ts"] button').click();
  await expect(page.locator('#pg-code')).toContainText('listPets');
  await replaceText(page, '#pg-config', SNAKE_CASE_CONFIG);
  await expect(page.locator('#pg-code')).toContainText('list_pets');
  await expect(page.locator('#pg-diagnostics li.is-note')).toContainText('input and output are ignored');
});

test('reports a broken config in the console and keeps the last output', async ({ page }) => {
  await ready(page);
  await replaceText(page, '#pg-config', '{"emit": [');
  await expect(page.locator('#pg-diagnostics li.is-error')).toContainText('config is not valid JSON');
  await expect(page.locator('.pg')).toHaveClass(/is-stale/);
  await replaceText(page, '#pg-config', '{"emit": ["models"], "typo": true}');
  await expect(page.locator('#pg-diagnostics li.is-error')).toContainText('E_INVALID_OPTION');
});

test('lays the panes out untouched by the prose styles', async ({ page }) => {
  await ready(page);
  const paneTops = await page
    .locator('.pg-panes > *')
    .evaluateAll(els => els.map(el => el.getBoundingClientRect().top));
  expect(new Set(paneTops).size).toBe(1);
  const buttonHeights = await page
    .locator('.pg-actions button')
    .evaluateAll(els => els.map(el => el.getBoundingClientRect().height));
  expect(new Set(buttonHeights).size).toBe(1);
  await expect(page.locator('.cm-line').nth(1)).toHaveCSS('margin-top', '0px');
  const [pgWidth, panelWidth] = await page.evaluate(() => {
    const panel = document.querySelector('.content-panel') as HTMLElement;
    const panelStyle = getComputedStyle(panel);
    return [
      document.querySelector('.pg')!.getBoundingClientRect().width,
      panel.clientWidth - parseFloat(panelStyle.paddingLeft) - parseFloat(panelStyle.paddingRight),
    ];
  });
  expect(pgWidth).toBeCloseTo(panelWidth, 0);
  const tree = page.locator('#pg-tree');
  const [treeScrollWidth, treeClientWidth] = await tree.evaluate(el => [el.scrollWidth, el.clientWidth]);
  expect(treeScrollWidth).toBe(treeClientWidth);
  const console = page.locator('#pg-console');
  await expect(console).toHaveCSS('overflow-y', 'auto');
  expect(await console.evaluate(el => parseFloat(getComputedStyle(el).maxHeight))).toBeGreaterThan(0);
});

test.describe('on a wide screen', () => {
  test.use({ viewport: { width: 2560, height: 1300 } });

  test('fills the width whatever file is selected', async ({ page }) => {
    await ready(page);
    const widthOf = () => page.locator('.pg').evaluate(el => el.getBoundingClientRect().width);
    await page.locator('#pg-tree li[data-path="rest/pet.rest.generated.ts"] button').click();
    const wide = await widthOf();
    await page.locator('#pg-tree li[data-path="rest.util.ts"] button').click();
    expect(await widthOf()).toBeCloseTo(wide, 0);
    const panelWidth = await page.evaluate(() => {
      const panel = document.querySelector('main > .content-panel:last-child') as HTMLElement;
      const style = getComputedStyle(panel);
      return panel.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight);
    });
    expect(wide).toBeCloseTo(panelWidth, 0);
  });
});

test('scrolls inside each pane, never in a nested wrapper', async ({ page }) => {
  await ready(page);
  await page.locator('#pg-tree li[data-path="rest/pet.rest.generated.ts"] button').click();
  // CodeMirror owns its scrolling; an outer scroller would double up and re-measure.
  for (const pane of ['#pg-editor', '#pg-config']) {
    await expect(page.locator(pane)).toHaveCSS('overflow-x', 'hidden');
    const [scrollWidth, clientWidth] = await page
      .locator(pane)
      .evaluate(el => [el.scrollWidth, el.clientWidth]);
    expect(scrollWidth).toBe(clientWidth);
  }
  // The output pane is the single scroller: the code element must not scroll too.
  await expect(page.locator('.pg-output pre code')).toHaveCSS('overflow-x', 'visible');
  const stable = await page.locator('.pg-output').evaluate(async el => {
    const before = el.scrollWidth;
    el.scrollLeft = el.scrollWidth;
    await new Promise(r => setTimeout(r, 200));
    return [before, el.scrollWidth];
  });
  expect(stable[1]).toBe(stable[0]);
});

test('shows a themed selection in the config editor', async ({ page }) => {
  await ready(page);
  const selectionBackground = async (theme: 'light' | 'dark') => {
    await page.evaluate(t => document.documentElement.dataset.theme = t, theme);
    const box = (await page.locator('#pg-config .cm-content').boundingBox())!;
    await page.mouse.move(box.x + 10, box.y + 8);
    await page.mouse.down();
    await page.mouse.move(box.x + 180, box.y + 34, { steps: 8 });
    await page.mouse.up();
    return page.locator('#pg-config .cm-selectionLayer .cm-selectionBackground').first().evaluate(
      el => getComputedStyle(el).backgroundColor,
    );
  };
  const light = await selectionBackground('light');
  const dark = await selectionBackground('dark');
  expect(light).not.toBe(dark);
});
