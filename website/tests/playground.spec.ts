import { expect, test, type Page } from '@playwright/test';
import { DEFAULT_CONFIG } from '../src/playground/config';
import { uncommentConfig } from './uncomment';

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
  await expect(error).toContainText('not valid JSONC');
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

const NO_TAGS_SPEC = [
  'openapi: 3.0.3',
  'info: {title: untagged, version: "1"}',
  'paths:',
  '  /store-items/{itemId}:',
  '    get:',
  '      operationId: fetch_storeItem',
  '      parameters: [{name: itemId, in: path, required: true, schema: {type: string}}]',
  '      responses: {"200": {description: ok}}',
].join('\n');

test('generates the same files with every default option enabled in the config', async ({ page }) => {
  await ready(page);
  const artifacts = () =>
    page.locator('#pg-tree li[data-path]').evaluateAll(els => els.map(el => el.getAttribute('data-path')));
  // The editor renders only the viewport, so the tree's byte size covers the rest.
  const codeOf = async (path: string) => {
    const row = page.locator(`#pg-tree li[data-path="${path}"]`);
    await row.locator('button').click();
    return `${await row.locator('.size').innerText()}\n${await page.locator('#pg-code .cm-content').innerText()}`;
  };
  const generated = async (edit: () => Promise<void>) => {
    const before = await page.locator('.pg').getAttribute('data-run');
    await edit();
    await expect(page.locator('.pg')).not.toHaveAttribute('data-run', before ?? '');
    await expect(page.locator('#pg-diagnostics li.is-error')).toHaveCount(0);
  };
  for (const spec of [null, NO_TAGS_SPEC]) {
    await generated(async () => {
      if (spec) await replaceText(page, '#pg-editor', spec);
      await replaceText(page, '#pg-config', DEFAULT_CONFIG);
    });
    const paths = await artifacts();
    const expected = new Map<string, string>();
    for (const path of paths) expected.set(path!, await codeOf(path!));
    await generated(() => replaceText(page, '#pg-config', uncommentConfig(DEFAULT_CONFIG)));
    expect(await artifacts()).toEqual(paths);
    for (const [path, code] of expected) expect(await codeOf(path)).toBe(code);
  }
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
    const panel = document.querySelector('main > .content-panel:last-child') as HTMLElement;
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
  // The output editor scrolls inside its own scroller as well.
  await expect(page.locator('.pg-output')).toHaveCSS('overflow-x', 'hidden');
  const [outputScrollWidth, outputClientWidth] = await page
    .locator('.pg-output')
    .evaluate(el => [el.scrollWidth, el.clientWidth]);
  expect(outputScrollWidth).toBe(outputClientWidth);
});

test('opens a spec file from disk or by dropping it on the editor', async ({ page }) => {
  await ready(page);
  await page.locator('#pg-file').setInputFiles({
    name: 'cookies.yaml',
    mimeType: 'application/yaml',
    buffer: Buffer.from(COOKIE_PARAM_SPEC),
  });
  await expect(page.locator('#pg-editor .cm-content')).toContainText('operationId: getA');
  await expect(page.locator('#pg-diagnostics li.is-warning')).toContainText('unsupported-parameter-location');
  // Dropping a file replaces the document instead of splicing it in at the cursor.
  await page.locator('#pg-editor .cm-content').evaluate((el, spec) => {
    const transfer = new DataTransfer();
    transfer.items.add(new File([spec], 'invalid.yaml', { type: 'application/yaml' }));
    el.dispatchEvent(new DragEvent('drop', { dataTransfer: transfer, bubbles: true, cancelable: true }));
  }, INVALID_SPEC);
  await expect(page.locator('#pg-editor .cm-content')).not.toContainText('getA');
  await expect(page.locator('#pg-editor .cm-content')).toContainText('/a: {get:');
  await expect(page.locator('#pg-diagnostics li.is-error')).toContainText('missing-operation-id');
});

test('switches between docs and playground from the header', async ({ page }) => {
  await ready(page);
  const sections = page.locator('header nav[aria-label="Site sections"]');
  await expect(sections.locator('a[aria-current="page"]')).toHaveText('Playground');
  await expect(page.locator('main h1')).toBeHidden();
  await expect(page.locator('header site-search')).toHaveCount(0);
  await sections.getByRole('link', { name: 'Docs' }).click();
  await expect(page).toHaveURL('/getting-started/');
  await expect(page.locator('header site-search')).toHaveCount(1);
  await expect(sections.locator('a[aria-current="page"]')).toHaveText('Docs');
  await expect(page.locator('.sidebar-content')).toContainText('Getting started');
  await expect(page.locator('.sidebar-content')).not.toContainText('Playground');
  await sections.getByRole('link', { name: 'Playground' }).click();
  await expect(page).toHaveURL('/playground/');
});

test('shows the generated file in a read-only editor with line numbers and search', async ({ page }) => {
  await ready(page);
  await page.locator('#pg-tree li[data-path="rest/pet.rest.generated.ts"] button').click();
  const output = page.locator('#pg-code');
  await expect(output.locator('.cm-content')).toHaveAttribute('contenteditable', 'false');
  await expect(output.locator('.cm-lineNumbers .cm-gutterElement').nth(1)).toHaveText('1');
  await expect(output.locator('.cm-content .tok-keyword').first()).toBeVisible();
  await output.locator('.cm-content').click();
  await page.keyboard.press('ControlOrMeta+f');
  const search = output.locator('.cm-search');
  await expect(search).toBeVisible();
  await search.locator('input[name="search"]').pressSequentially('listPets');
  await page.keyboard.press('Enter');
  await expect(output.locator('.cm-searchMatch-selected')).toHaveText('listPets');
  await output.locator('.cm-content').click();
  await page.keyboard.press('ControlOrMeta+a');
  await page.keyboard.type('x');
  await expect(output).toContainText('listPets');
});

test('paints the editors like the docs code blocks in both themes', async ({ page }) => {
  const themed = async (theme: 'light' | 'dark', selector: string) => {
    await page.evaluate(t => (document.documentElement.dataset.theme = t), theme);
    return page.locator(selector).first().evaluate(el => getComputedStyle(el).backgroundColor);
  };
  const colourOf = (selector: string) =>
    page.locator(selector).first().evaluate(el => getComputedStyle(el).color);
  await page.goto('/getting-started/');
  const snippet: Record<string, { background: string; keyword: string }> = {};
  for (const theme of ['light', 'dark'] as const) {
    snippet[theme] = {
      background: await themed(theme, '.expressive-code pre'),
      keyword: await colourOf('.expressive-code pre span:text-is("import")'),
    };
  }
  expect(snippet.light.background).not.toBe(snippet.dark.background);
  await ready(page);
  await page.locator('#pg-tree li[data-path="rest/pet.rest.generated.ts"] button').click();
  for (const theme of ['light', 'dark'] as const) {
    for (const pane of ['#pg-editor', '#pg-config', '#pg-code']) {
      expect(await themed(theme, `${pane} .cm-editor`)).toBe(snippet[theme].background);
    }
    expect(await colourOf('#pg-code .tok-keyword')).toBe(snippet[theme].keyword);
  }
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
