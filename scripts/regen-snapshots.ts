#!/usr/bin/env bun
// Regenerates __test__/snapshots/generate-native/ by running each fixture
// through `generate` and storing the result.
//
// Storage layout:
//   <fixture>.success.json   summary, diagnostics, and a path-only artifact
//                            list — no inline contents
//   <fixture>/<path>         each artifact's body as a sibling file, so a
//                            PR diff reads as TypeScript rather than as
//                            JSON-escaped strings
//   static-template.json     the path-only list for the Angular support
//   static-template/<path>   files, whose bodies are identical across
//                            every success fixture and so stored once
//
// Every fixture in test/fixtures/ must appear in exactly one of the three
// sets in scripts/lib/snapshot-layout.ts; the script fails on one that
// appears in none.
//
// Run with: bun run regen-snapshots

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { generate, isGenerateError } from './lib/engine.ts';
import type { GenerateError, GenerateOptions, GenerateResult } from './lib/engine.ts';
import {
  BANNER_RE,
  FAILURE_FIXTURES,
  SNAPSHOT_EMIT,
  STATIC_TEMPLATE_PATHS,
  SUCCESS_FIXTURES,
  UNSNAPSHOTTED,
  snapshotDir,
} from './lib/snapshot-layout.ts';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const fixturesDir = path.join(repoRoot, 'test', 'fixtures');
const snapshots = snapshotDir(repoRoot);
const staticTemplateDir = path.join(snapshots, 'static-template');
const staticTemplateIndex = path.join(snapshots, 'static-template.json');

/**
 * Fails when a fixture on disk appears in none of the three sets, so a new
 * fixture must be classified rather than silently ignored.
 */
function assertEveryFixtureIsClassified(): void {
  const classified = new Set<string>([
    ...SUCCESS_FIXTURES.map(entry => entry.fixture),
    ...FAILURE_FIXTURES.map(entry => entry.fixture),
    ...UNSNAPSHOTTED,
  ]);
  const unclassified = fs
    .readdirSync(fixturesDir)
    .filter(name => /\.(ya?ml|json)$/u.test(name))
    .filter(name => !classified.has(name));

  if (unclassified.length > 0) {
    console.error(
      `regen-snapshots: ${unclassified.length} fixture(s) appear in none of ` +
        'SUCCESS_FIXTURES, FAILURE_FIXTURES or UNSNAPSHOTTED:\n' +
        unclassified.map(name => `  ${name}`).join('\n'),
    );
    process.exit(1);
  }
}

let updated = 0;
let unchanged = 0;

function writeIfChanged(target: string, contents: string): void {
  const previous = fs.existsSync(target) ? fs.readFileSync(target, 'utf8') : null;
  if (previous === contents) {
    unchanged += 1;
    return;
  }
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.writeFileSync(target, contents);
  updated += 1;
  console.log(`updated: ${path.relative(repoRoot, target)}`);
}

/** Removes files under `dir` that the current run did not write. */
function removeOrphans(dir: string, live: ReadonlySet<string>): void {
  if (!fs.existsSync(dir)) return;

  const walk = (current: string, prefix: string): void => {
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const child = path.join(current, entry.name);
      const relative = prefix ? path.join(prefix, entry.name) : entry.name;
      if (entry.isDirectory()) {
        walk(child, relative);
        if (fs.readdirSync(child).length === 0) fs.rmdirSync(child);
      } else if (!live.has(relative)) {
        fs.rmSync(child);
        console.log(`removed: ${path.relative(repoRoot, child)}`);
      }
    }
  };
  walk(dir, '');
}

function storeBodies(
  dir: string,
  artifacts: GenerateResult['artifacts'],
  keep: (artifactPath: string) => boolean,
): void {
  const live = new Set<string>();
  for (const artifact of artifacts) {
    if (!keep(artifact.path)) continue;
    const target = path.join(dir, artifact.path);
    writeIfChanged(target, artifact.contents.replace(BANNER_RE, ''));
    live.add(path.relative(dir, target));
  }
  removeOrphans(dir, live);
}

function asJson(value: unknown): string {
  return `${JSON.stringify(value, null, 2)}\n`;
}

function writeSuccessSnapshot(fixture: string, result: GenerateResult): void {
  storeBodies(path.join(snapshots, fixture), result.artifacts, artifactPath =>
    !STATIC_TEMPLATE_PATHS.has(artifactPath),
  );
  writeIfChanged(
    path.join(snapshots, `${fixture}.success.json`),
    asJson({
      summary: result.summary,
      diagnostics: result.diagnostics,
      artifacts: result.artifacts.map(artifact => ({ path: artifact.path })),
    }),
  );
}

function writeStaticTemplates(result: GenerateResult): void {
  storeBodies(staticTemplateDir, result.artifacts, artifactPath =>
    STATIC_TEMPLATE_PATHS.has(artifactPath),
  );
  writeIfChanged(
    staticTemplateIndex,
    asJson({
      artifacts: [...STATIC_TEMPLATE_PATHS].sort().map(templatePath => ({ path: templatePath })),
    }),
  );
}

/** The failure fields a snapshot pins, in a stable key order. */
function failurePayload(error: GenerateError) {
  return {
    code: error.code,
    message: error.message,
    path: error.path,
    warnings: error.warnings,
  };
}

/** Rethrows anything that is not one of the generator's own failures. */
function asGenerateError(error: unknown, fixture: string): GenerateError {
  if (isGenerateError(error)) return error;
  throw new Error(`${fixture} failed with a non-generator error`, { cause: error });
}

function run(
  fixture: string,
  options: Partial<GenerateOptions> = {},
): Promise<GenerateResult> {
  return generate({
    inputPath: path.join('test', 'fixtures', fixture),
    emit: [...SNAPSHOT_EMIT],
    ...options,
  });
}

assertEveryFixtureIsClassified();

let staticTemplatesWritten = false;
for (const { fixture } of SUCCESS_FIXTURES) {
  let result;
  try {
    result = await run(fixture);
  } catch (error) {
    console.error(
      `FAIL: ${fixture} was expected to generate but failed with ` +
        `${asGenerateError(error, fixture).code}. Add it to FAILURE_FIXTURES, or fix the fixture.`,
    );
    process.exitCode = 1;
    continue;
  }
  if (!staticTemplatesWritten) {
    writeStaticTemplates(result);
    staticTemplatesWritten = true;
  }
  writeSuccessSnapshot(fixture, result);
}

for (const { fixture, snapshot, options } of FAILURE_FIXTURES) {
  try {
    await run(fixture, options);
    console.warn(`SKIP: ${fixture} (${snapshot}) succeeded — failure snapshot not regenerated`);
  } catch (error) {
    writeIfChanged(
      path.join(snapshots, snapshot),
      asJson(failurePayload(asGenerateError(error, fixture))),
    );
  }
}

console.log(`\n${updated} snapshot(s) updated, ${unchanged} unchanged.`);
