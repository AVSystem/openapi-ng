import test from 'ava';
import fs from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { generate as nativeGenerate } from '../lib/index.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.join(__dirname, '..');
const require = createRequire(import.meta.url);

type GenerateFn = (options: unknown) => Promise<{
  artifacts: Array<{ path: string; contents: string }>;
  diagnostics: unknown[];
}>;
type BrowserEntry = {
  generate: GenerateFn;
  createGenerate: (load: () => Promise<unknown>) => GenerateFn;
  GenerateError: { isGenerateError: (value: unknown) => boolean };
  EmitTarget: { Models: string; Angular: string };
};
type TypedError = { code?: string; subcode?: string | null; message: string };

const browserEntry = require(path.join(repoRoot, 'browser.js')) as BrowserEntry;
const wasiCjsPath = path.join(repoRoot, 'openapi-ng.wasi.cjs');
// `import()` needs a URL, not a bare Windows path.
const wasiCjs = pathToFileURL(wasiCjsPath).href;
const hasWasi = fs.existsSync(wasiCjsPath);
// CI's cross-check job downloads both bindings, so a skipped WASI test there
// means the artifact went missing rather than that the run is a local one.
const expectWasi = process.env.OPENAPI_NG_EXPECT_WASI_BINDING === '1';
if (expectWasi && !hasWasi) {
  test('WASI binding is present when OPENAPI_NG_EXPECT_WASI_BINDING is set', t => {
    t.fail(`expected ${wasiCjsPath} to exist`);
  });
}
const wasiTest = hasWasi ? test : test.skip;
const petstore = fs.readFileSync(
  path.join(repoRoot, 'test', 'fixtures', 'petstore-minimal.openapi.yaml'),
  'utf8',
);
const petstoreOptions = {
  inputContents: petstore,
  displayPath: 'petstore-minimal.openapi.yaml',
  emit: ['models', 'angular'],
};

wasiTest(
  'browser generate through the WASI binding matches the native output',
  async t => {
    const generate = browserEntry.createGenerate(() => import(wasiCjs));
    const [fromWasi, fromNative] = await Promise.all([
      generate(petstoreOptions),
      nativeGenerate(petstoreOptions),
    ]);
    t.deepEqual(
      fromWasi.artifacts.map(a => [a.path, a.contents]),
      fromNative.artifacts.map(a => [a.path, a.contents]),
    );
    t.deepEqual(fromWasi.diagnostics, fromNative.diagnostics);
  },
);

wasiTest('browser generate surfaces typed fatal diagnostics', async t => {
  const generate = browserEntry.createGenerate(() => import(wasiCjs));
  const err = (await t.throwsAsync(() =>
    generate({
      inputContents:
        'openapi: 3.0.3\ninfo: {title: x, version: "1"}\npaths: {/a: {get: {responses: {"200": {description: ok}}}}}',
      displayPath: 'x.yaml',
      emit: ['models', 'angular'],
    }),
  )) as TypedError;
  t.true(browserEntry.GenerateError.isGenerateError(err));
  t.is(err.code, 'E_POLICY_VIOLATION');
  t.is(err.subcode, 'missing-operation-id');
});

test('browser generate rejects inputPath with E_INVALID_OPTION', async t => {
  const err = (await t.throwsAsync(() =>
    browserEntry.generate({ inputPath: 'spec.yaml', emit: ['models'] }),
  )) as TypedError;
  t.is(err.code, 'E_INVALID_OPTION');
  t.is(err.subcode, 'shape');
  t.regex(err.message, /inputContents/);
});

test('browser generate rejects outputPath with E_INVALID_OPTION', async t => {
  const err = (await t.throwsAsync(() =>
    browserEntry.generate({ ...petstoreOptions, outputPath: 'out' }),
  )) as TypedError;
  t.is(err.code, 'E_INVALID_OPTION');
  t.is(err.subcode, 'shape');
  t.regex(err.message, /outputPath/);
});

test('browser generate maps a failed binding load to E_UNSUPPORTED_RUNTIME', async t => {
  const generate = browserEntry.createGenerate(() =>
    Promise.reject(new Error('no wasm here')),
  );
  const err = (await t.throwsAsync(() => generate(petstoreOptions))) as TypedError;
  t.is(err.code, 'E_UNSUPPORTED_RUNTIME');
  t.regex(err.message, /openapi-ng-wasm32-wasi/);
  t.regex(err.message, /no wasm here/);
});

test('browser generate maps a binding without generateNative to E_UNSUPPORTED_RUNTIME', async t => {
  const generate = browserEntry.createGenerate(() => Promise.resolve({}));
  const err = (await t.throwsAsync(() => generate(petstoreOptions))) as TypedError;
  t.is(err.code, 'E_UNSUPPORTED_RUNTIME');
  t.regex(err.message, /generateNative/);
});

test('browser entry exports EmitTarget mirror', t => {
  t.is(browserEntry.EmitTarget.Models, 'models');
  t.is(browserEntry.EmitTarget.Angular, 'angular');
});
