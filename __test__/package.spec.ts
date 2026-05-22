import test from 'ava';
import fs from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { GenerateError } from '../lib/index.js';
import * as lib from '../lib/index.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.join(__dirname, '..');
const require = createRequire(import.meta.url);

test('browser entry exists and fails with an explicit unsupported-runtime error', async t => {
  const browserEntry = require(path.join(repoRoot, 'browser.js')) as {
    generate: (options?: unknown) => Promise<never>;
  };

  t.is(typeof browserEntry.generate, 'function');

  // The browser entry is a hard-error stub — browser/edge runtimes are
  // unsupported, so generate() rejects with E_UNSUPPORTED_RUNTIME at call
  // time. The module itself must stay importable so bundlers don't choke.
  const generateError = (await t.throwsAsync(async () => {
    await browserEntry.generate();
  })) as GenerateError | undefined;
  t.is(generateError?.code, 'E_UNSUPPORTED_RUNTIME');
  t.regex(generateError?.message ?? '', /browser|runtime/i);
});

test('package.json exports map covers node, browser, default, and types conditions', t => {
  const packageJson = JSON.parse(
    fs.readFileSync(path.join(repoRoot, 'package.json'), 'utf8'),
  ) as {
    exports?: Record<string, Record<string, string> | string>;
  };

  const root = packageJson.exports?.['.'];
  t.truthy(root, 'package.json exports map must include "."');
  t.is(typeof root, 'object');
  const conditions = root as Record<string, string>;
  t.is(conditions.types, './index.d.ts');
  t.is(conditions.browser, './browser.js');
  // Node entry is the wrapper at lib/index.js that upgrades thrown
  // errors into `GenerateError` instances. The raw napi-rs binding at
  // ./index.js is internal.
  t.is(conditions.node, './lib/index.js');
  t.is(conditions.default, './lib/index.js');
});

test('package metadata keeps the node-only packaging contract explicit', t => {
  const packageJson = JSON.parse(
    fs.readFileSync(path.join(repoRoot, 'package.json'), 'utf8'),
  ) as {
    browser?: string;
    description?: string;
    files?: string[];
    main?: string;
    types?: string;
  };

  t.is(packageJson.main, 'lib/index.js');
  t.is(packageJson.types, 'index.d.ts');
  t.is(packageJson.browser, 'browser.js');
  t.true(packageJson.files?.includes('browser.js') ?? false);
  t.true(packageJson.files?.includes('lib/index.js') ?? false);
  t.notRegex(packageJson.description ?? '', /Template project/i);
});

test('napi.targets is non-empty and lists only native triples (no wasm)', t => {
  const packageJson = JSON.parse(
    fs.readFileSync(path.join(repoRoot, 'package.json'), 'utf8'),
  ) as { napi?: { targets?: string[] } };
  const targets = packageJson.napi?.targets ?? [];
  t.true(targets.length > 0, 'napi.targets must be non-empty');
  for (const triple of targets) {
    t.false(triple.startsWith('wasm32-'), `unexpected wasm32 target: ${triple}`);
  }
});

test('GenerateError always exposes code as a string', t => {
  const err = new GenerateError({ message: 'm' });
  t.is(typeof err.code, 'string');
  t.not(err.code, undefined);
});

test('GenerateError always exposes path as a string', t => {
  const err = new GenerateError({ message: 'm' });
  t.is(typeof err.path, 'string');
  t.not(err.path, undefined);
});

test('GenerateError.subcode is null (never undefined) when absent', t => {
  const err = new GenerateError({ message: 'm' });
  t.is(err.subcode, null);
});

test('public surface is a fixed allow-list', t => {
  const allowed = new Set(['generate', 'GenerateError', 'EmitTarget']);
  // Node's CJS-to-ESM interop synthesises two bindings on `import * as`:
  //   - `'module.exports'`: the raw CJS object alongside per-property exports;
  //   - `'default'`: the same CJS object exposed as the default import.
  // Neither is under the wrapper's control, so filter both out here.
  // The wrapper-controlled absence of a default export is asserted via
  // CommonJS `require()` in the dedicated test below.
  const actual = new Set(
    Object.keys(lib).filter(k => k !== 'module.exports' && k !== 'default'),
  );
  for (const key of actual) {
    t.true(allowed.has(key), `unexpected export: ${key}`);
  }
});

test('public surface does not advertise a default export', t => {
  // Read via CommonJS `require()` so we see the wrapper's actual
  // `module.exports` object — `import * as` would synthesise a
  // `default` binding via Node's CJS-to-ESM interop regardless of what
  // the wrapper exposes, hiding the very thing we're asserting.
  const mod = require('../lib/index.js');
  t.false('default' in mod, 'Node entry should not expose a default export');
});

test('engines.node declares a >=18 floor', t => {
  const packageJson = JSON.parse(
    fs.readFileSync(path.join(repoRoot, 'package.json'), 'utf8'),
  ) as { engines?: { node?: string } };

  const range = packageJson.engines?.node ?? '';
  // 18 is the floor pinned by the supported-Node policy (CI matrix, README,
  // and native-binding ABI assumptions). Installs on older Node should fail
  // at the manifest boundary, not at first CLI invocation.
  const match = range.match(/>=\s*(\d+)/);
  t.truthy(match, `engines.node must declare a >= lower bound (got '${range}')`);
  const major = Number(match![1]);
  t.true(major >= 18, `engines.node lower bound must be >= 18, got '${range}'`);
});

test('package.json engines.node matches README', t => {
  const pkg = JSON.parse(
    fs.readFileSync(path.join(repoRoot, 'package.json'), 'utf8'),
  ) as { engines?: { node?: string } };
  const readme = fs.readFileSync(path.join(repoRoot, 'README.md'), 'utf8');
  t.is(pkg.engines?.node, '>=18.0.0');
  t.regex(readme, /Requires Node\.js >= 18/);
});

test('patch-types narrows GeneratorDiagnostic and GenerateErrorPayload bodies', t => {
  const dts = fs.readFileSync(path.join(repoRoot, 'index.d.ts'), 'utf8');

  // Each narrowed line must appear exactly once in the generated d.ts
  // (twice in total — once per interface — for the shared lines).
  function countOccurrences(haystack: string, needle: string): number {
    return haystack.split(needle).length - 1;
  }

  t.is(countOccurrences(dts, '  code: DiagnosticCode'), 2);
  t.is(countOccurrences(dts, '  subcode: DiagnosticSubcode | null'), 2);
  t.is(countOccurrences(dts, "  severity: 'warning' | 'error'"), 1);

  // The unpatched literals must not survive in the published surface.
  t.false(dts.includes('  subcode?: string'));
  t.false(dts.includes('  code: string'));
  t.false(dts.includes('  severity: string'));
});

test('runtime docs document the current node-only boundary and browser stub', t => {
  const runtimeDoc = fs.readFileSync(
    path.join(repoRoot, 'website', 'src', 'content', 'docs', 'reference', 'runtime.md'),
    'utf8',
  );

  t.regex(runtimeDoc, /does not support browser runtimes/i);
  t.regex(runtimeDoc, /E_UNSUPPORTED_RUNTIME/);
});

test('native.js includes a friendly unsupported-platform error with supported list', t => {
  const nativeJs = fs.readFileSync(path.join(repoRoot, 'native.js'), 'utf8');

  t.true(
    nativeJs.includes('does not ship a native binary for'),
    'native.js must include the friendly unsupported-platform error message',
  );

  // Each supported platform key must be listed in the injected Set.
  for (const platformKey of [
    'darwin/x64',
    'darwin/arm64',
    'linux/x64',
    'linux/arm64',
    'win32/x64',
    'win32/arm64',
  ]) {
    t.true(
      nativeJs.includes(`'${platformKey}'`),
      `native.js must list '${platformKey}' as a supported platform`,
    );
  }
});

test('lib/config.js is shipped in package.json files allow-list', t => {
  const packageJson = JSON.parse(
    fs.readFileSync(path.join(repoRoot, 'package.json'), 'utf8'),
  ) as { files?: string[] };
  t.true(
    packageJson.files?.includes('lib/config.js') ?? false,
    'package.json files must include lib/config.js',
  );
});

test('package.json exports map exposes ./config subpath', t => {
  const packageJson = JSON.parse(
    fs.readFileSync(path.join(repoRoot, 'package.json'), 'utf8'),
  ) as { exports?: Record<string, Record<string, string> | string> };
  const subpath = packageJson.exports?.['./config'];
  t.truthy(subpath, 'package.json exports must include "./config"');
  t.is(typeof subpath, 'object');
  const conditions = subpath as Record<string, string>;
  t.is(conditions.types, './index.d.ts');
  t.is(conditions.default, './lib/config.js');
});

test('@avsystem/openapi-ng/config exports defineConfig as an identity function', t => {
  const mod = require(path.join(repoRoot, 'lib', 'config.js')) as {
    defineConfig: <T>(c: T) => T;
  };
  t.is(typeof mod.defineConfig, 'function');
  const input = { input: 'spec.yaml', output: './out' };
  t.is(mod.defineConfig(input), input);
});
