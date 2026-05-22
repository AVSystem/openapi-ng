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

  // The browser entry is now a real async wrapper; generate() rejects
  // with a GenerateError when the optional
  // `@avsystem/openapi-ng-wasm32-wasip1-threads` package can't load
  // (typical on a dev machine without the WASI sub-package installed).
  // The message names the optional package so the consumer knows what
  // to install.
  const generateError = await t.throwsAsync(async () => {
    await browserEntry.generate();
  });
  const msg = generateError?.message ?? '';
  t.regex(msg, /browser|runtime/i);
  t.true(
    msg.includes('@avsystem/openapi-ng-wasm32-wasip1-threads'),
    `message must name the optional WASI package: ${msg}`,
  );
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

/**
 * Pure mapping from a Rust target triple to the npm sub-package name that
 * `napi prepublish -t npm` publishes. Mirrors the canonical napi-rs naming
 * scheme so the optionalDependencies block can be derived from `napi.targets`.
 * `packageName` is the host package's full name (including any `@scope/`),
 * which napi-rs uses as the prefix for every sub-package.
 */
function napiSubPackageName(packageName: string, triple: string): string {
  const archMap: Record<string, string> = {
    x86_64: 'x64',
    aarch64: 'arm64',
    i686: 'ia32',
    armv7: 'arm',
  };

  // WASI/WASM targets keep their full Rust triple as the sub-package suffix
  // (napi-rs convention; e.g. `@avsystem/openapi-ng-wasm32-wasip1-threads`).
  if (triple.startsWith('wasm32-')) {
    return `${packageName}-${triple}`;
  }

  const [rawArch, ...rest] = triple.split('-');
  const arch = archMap[rawArch] ?? rawArch;
  const remainder = rest.join('-');

  if (remainder === 'apple-darwin') {
    return `${packageName}-darwin-${arch}`;
  }
  if (remainder === 'unknown-linux-gnu') {
    return `${packageName}-linux-${arch}-gnu`;
  }
  if (remainder === 'unknown-linux-musl') {
    return `${packageName}-linux-${arch}-musl`;
  }
  if (remainder === 'pc-windows-msvc') {
    return `${packageName}-win32-${arch}-msvc`;
  }
  if (remainder === 'pc-windows-gnu') {
    return `${packageName}-win32-${arch}-gnu`;
  }
  throw new Error(`Unsupported napi target triple: ${triple}`);
}

test('optionalDependencies covers every napi target', t => {
  const packageJson = JSON.parse(
    fs.readFileSync(path.join(repoRoot, 'package.json'), 'utf8'),
  ) as {
    name?: string;
    napi?: { binaryName?: string; packageName?: string; targets?: string[] };
    optionalDependencies?: Record<string, string>;
  };

  const targets = packageJson.napi?.targets ?? [];
  // napi-rs derives sub-package names from `napi.packageName ?? pkg.name`,
  // NOT from `napi.binaryName` (which only controls the `.node` filename).
  // Mirror that so a scoped host package (e.g. `@avsystem/openapi-ng`) maps
  // to scoped sub-packages.
  const packageName = packageJson.napi?.packageName ?? packageJson.name ?? '';
  t.true(targets.length > 0, 'napi.targets must be non-empty');
  t.truthy(packageName, 'package.json#name (or napi.packageName) must be set');

  const expected = [
    ...new Set(targets.map(triple => napiSubPackageName(packageName, triple))),
  ].sort();
  const actual = [...new Set(Object.keys(packageJson.optionalDependencies ?? {}))].sort();

  t.deepEqual(
    actual,
    expected,
    'optionalDependencies must list exactly the napi-derived sub-package names',
  );
});

test('optionalDependency versions track package.json#version', t => {
  const packageJson = JSON.parse(
    fs.readFileSync(path.join(repoRoot, 'package.json'), 'utf8'),
  ) as {
    version: string;
    optionalDependencies?: Record<string, string>;
  };

  for (const [name, version] of Object.entries(packageJson.optionalDependencies ?? {})) {
    t.is(
      version,
      packageJson.version,
      `${name} pinned to a different version than the host`,
    );
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

test('runtime docs document the current node-only boundary and thin native transport', t => {
  const runtimeDoc = fs.readFileSync(
    path.join(
      repoRoot,
      'website',
      'src',
      'content',
      'docs',
      'reference',
      'runtime.md',
    ),
    'utf8',
  );

  t.regex(runtimeDoc, /does not support browser runtimes/i);
  t.regex(
    runtimeDoc,
    /thin transport over the same\s+native `generate\(\)` path used by Node/i,
  );
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
