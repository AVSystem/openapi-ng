import test from 'ava';
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

import { generate, isGenerateError } from '../scripts/lib/engine.ts';
import type { GenerateOptions } from '../scripts/lib/engine.ts';
import {
  BANNER_RE,
  FAILURE_FIXTURES,
  SNAPSHOT_EMIT,
  STATIC_TEMPLATE_PATHS,
  SUCCESS_FIXTURES,
} from '../scripts/lib/snapshot-layout.ts';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.join(__dirname, '..');
// Pass fixtures as paths relative to CWD (project root) so display_path
// returns a short, machine-independent path in snapshots and banners.
const fixture = (name: string) => path.join('test', 'fixtures', name);
const snapshot = (name: string) =>
  path.join(repoRoot, '__test__', 'snapshots', 'generate-native', name);

function readJsonSnapshot(name: string) {
  return JSON.parse(fs.readFileSync(snapshot(name), 'utf8'));
}

const STATIC_TEMPLATE_DIR = path.join(
  repoRoot,
  '__test__',
  'snapshots',
  'generate-native',
  'static-template',
);

/**
 * Hydrate a path-only success snapshot into the full shape the live
 * `generate` result has: each non-static artifact picks up its
 * `contents` from `<snapshot-dir>/<fixture>/<path>`; static-template
 * artifacts remain path-only because their bodies are pinned in
 * static-template.json and asserted by `staticTemplateArtifacts` below.
 */
function hydrateSuccessSnapshot(fixtureName: string) {
  const snap = readJsonSnapshot(`${fixtureName}.success.json`);
  const siblingsDir = path.join(
    repoRoot,
    '__test__',
    'snapshots',
    'generate-native',
    fixtureName,
  );
  const artifacts = snap.artifacts.map((a: any) => {
    if (STATIC_TEMPLATE_PATHS.has(a.path)) {
      return { path: a.path };
    }
    const filePath = path.join(siblingsDir, a.path);
    const contents = fs.readFileSync(filePath, 'utf8');
    return { path: a.path, contents };
  });
  return { ...snap, artifacts };
}

/**
 * Hydrate the static-template snapshot: list of {path, contents}
 * loaded from <snapshot-dir>/static-template/<path>. Sorted by path so
 * the deep-equal stays insensitive to OS readdir order.
 */
function hydrateStaticTemplate() {
  const snap = readJsonSnapshot('static-template.json');
  const artifacts = snap.artifacts
    .map((a: any) => ({
      path: a.path,
      contents: fs.readFileSync(path.join(STATIC_TEMPLATE_DIR, a.path), 'utf8'),
    }))
    .sort((a: any, b: any) => a.path.localeCompare(b.path));
  return { artifacts };
}

function normalizeSuccessResult(result: any) {
  return {
    ...result,
    artifacts: result.artifacts.map((a: any) => {
      if (STATIC_TEMPLATE_PATHS.has(a.path)) {
        return { path: a.path };
      }
      return {
        ...a,
        contents:
          typeof a.contents === 'string' ? a.contents.replace(BANNER_RE, '') : a.contents,
      };
    }),
  };
}

function staticTemplateArtifacts(result: any) {
  return {
    artifacts: result.artifacts
      .filter((a: any) => STATIC_TEMPLATE_PATHS.has(a.path))
      .map((a: any) => ({ path: a.path, contents: a.contents.replace(BANNER_RE, '') }))
      .sort((a: any, b: any) => a.path.localeCompare(b.path)),
  };
}

async function successResult(name: string, options: Record<string, unknown> = {}) {
  return normalizeSuccessResult(
    await generate({
      inputPath: fixture(name),
      emit: ['models', 'angular'],
      ...options,
    }),
  );
}

async function failurePayload(name: string, options: Partial<GenerateOptions> = {}) {
  try {
    await generate({
      inputPath: fixture(name),
      emit: [...SNAPSHOT_EMIT],
      ...options,
    });
  } catch (thrown) {
    if (!isGenerateError(thrown)) {
      throw new Error(`${name} failed with a non-generator error`, { cause: thrown });
    }
    return {
      code: thrown.code,
      message: thrown.message,
      path: thrown.path,
      warnings: thrown.warnings,
    };
  }
  throw new Error(`Expected ${name} to fail.`);
}

for (const { fixture: fixtureName, pins } of SUCCESS_FIXTURES) {
  test(`${fixtureName} snapshot: ${pins}`, async t => {
    t.deepEqual(await successResult(fixtureName), hydrateSuccessSnapshot(fixtureName));
  });
}

// rest.model.ts and rest.util.ts are byte-identical across every success
// fixture. Pin them once here; per-fixture snapshots above keep only the
// `path` for these two artifacts. petstore-minimal is the smallest
// fixture that emits them, so it doubles as the baseline.
test('generate emits stable static-template artifacts (rest.model.ts, rest.util.ts, rest.validate.ts)', async t => {
  const baseline = await generate({
    inputPath: fixture('petstore-minimal.openapi.yaml'),
    emit: ['models', 'angular'],
  });
  t.deepEqual(staticTemplateArtifacts(baseline), hydrateStaticTemplate());
});

// Compile gate over the snapshots committed to disk: catches emitted TS
// that stays textually stable but stops compiling, such as an Angular
// service referencing an un-imported response type.
test('snapshot artifacts type-check under tsc --noEmit', t => {
  const repoNodeModules = path.join(repoRoot, 'node_modules');
  if (!fs.existsSync(path.join(repoNodeModules, 'typescript', 'bin', 'tsc'))) {
    t.fail('node_modules/typescript not installed — run `bun install` first');
    return;
  }

  // Under the angular-consumer fixture, so module resolution reaches the
  // repo's node_modules; a sibling of `generated/`, which
  // generate.spec.ts's reset helper wipes.
  const compileRoot = path.join(
    repoRoot,
    '__test__',
    'angular-consumer',
    '__snapshot_compile__',
  );
  fs.rmSync(compileRoot, { recursive: true, force: true });
  fs.mkdirSync(compileRoot, { recursive: true });

  const staticArtifacts: Array<{ path: string; contents: string }> =
    hydrateStaticTemplate().artifacts;

  const includeGlobs: string[] = [];
  for (const { fixture: fixtureName } of SUCCESS_FIXTURES) {
    const snap = hydrateSuccessSnapshot(fixtureName);
    const fixtureDir = path.join(
      compileRoot,
      fixtureName.replace(/[^a-zA-Z0-9_-]+/g, '_'),
    );
    fs.mkdirSync(fixtureDir, { recursive: true });

    for (const artifact of snap.artifacts) {
      // Per-fixture snapshots elide static-template `contents` (only the
      // `path` survives); pull bodies from the shared static-template
      // sibling files via `hydrateStaticTemplate`.
      const contents =
        typeof artifact.contents === 'string'
          ? artifact.contents
          : staticArtifacts.find(a => a.path === artifact.path)?.contents;
      if (contents === undefined) {
        t.fail(`Missing contents for ${artifact.path} in ${fixtureName}`);
        return;
      }
      const filePath = path.join(fixtureDir, artifact.path);
      fs.mkdirSync(path.dirname(filePath), { recursive: true });
      fs.writeFileSync(filePath, contents, 'utf8');
    }
    includeGlobs.push(`${path.basename(fixtureDir)}/**/*.ts`);
  }

  const compileTsconfig = {
    extends: '../tsconfig.json',
    include: includeGlobs,
  };
  fs.writeFileSync(
    path.join(compileRoot, 'tsconfig.json'),
    JSON.stringify(compileTsconfig, null, 2),
    'utf8',
  );

  execFileSync(
    process.execPath,
    [path.join(repoNodeModules, 'typescript', 'bin', 'tsc'), '-p', compileRoot],
    {
      cwd: compileRoot,
      stdio: 'pipe',
    },
  );
  t.pass('tsc --noEmit succeeded on every success-snapshot artifact set');
});

for (const { fixture: fixtureName, snapshot: snapshotName, pins, options } of FAILURE_FIXTURES) {
  test(`${snapshotName} snapshot: ${pins}`, async t => {
    t.deepEqual(
      await failurePayload(fixtureName, options ?? {}),
      readJsonSnapshot(snapshotName),
    );
  });
}

// malformed.yaml: pin code/path but use a regex on the message so
// upstream serde_yml line/column wording changes don't churn the snapshot.
test('generate preserves stable failure shape for malformed.yaml (regex message)', async t => {
  const payload = await failurePayload('malformed.yaml');
  t.is(payload.code, 'E_INPUT_INVALID');
  t.regex(payload.message, /^Failed to decode OpenAPI input as YAML:/);
  t.true((payload.path ?? '').endsWith('test/fixtures/malformed.yaml'));
  // No pre-fatal warnings expected for a decode-stage failure.
  t.deepEqual(payload.warnings, []);
});

