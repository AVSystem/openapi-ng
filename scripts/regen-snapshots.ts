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
// sets below; the script fails on one that appears in none.
//
// Run with: bun run regen-snapshots

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { generate, isGenerateError } from './lib/engine.ts';
import type { GenerateError, GenerateOptions, GenerateResult } from './lib/engine.ts';
import {
  BANNER_RE,
  SNAPSHOT_EMIT,
  STATIC_TEMPLATE_PATHS,
  snapshotDir,
} from './lib/snapshot-layout.ts';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const fixturesDir = path.join(repoRoot, 'test', 'fixtures');
const snapshots = snapshotDir(repoRoot);
const staticTemplateDir = path.join(snapshots, 'static-template');
const staticTemplateIndex = path.join(snapshots, 'static-template.json');

/** One failure snapshot: a fixture, the options it needs, and its label. */
interface FailureSnapshot {
  readonly fixture: string;
  readonly snapshot: string;
  readonly options?: Partial<GenerateOptions>;
}

/**
 * Fixtures that generate successfully and whose output is pinned.
 *
 * Ordered as the snapshots directory reads.
 */
const SUCCESS_FIXTURES: readonly string[] = [
  'additional-properties-false.openapi.yaml',
  'additional-properties.openapi.yaml',
  'allof-composition.openapi.yaml',
  'anchor-modest.openapi.yaml',
  'bench-large.openapi.yaml',
  'body-multipart-mixed-fields.openapi.yaml',
  'body-multipart-ref-to-named-object.openapi.yaml',
  'body-urlencoded-scalar-and-array.openapi.yaml',
  'circular-allof.openapi.yaml',
  'deprecated-fields.openapi.yaml',
  'discriminated-union.openapi.yaml',
  'discriminator-allof.openapi.yaml',
  'discriminator-mapping.openapi.yaml',
  'empty-shapes.openapi.yaml',
  'header-param.openapi.yaml',
  'inline-model.openapi.yaml',
  'jsdoc-descriptions.openapi.yaml',
  'large-enum.openapi.yaml',
  'multi-tag-operation.openapi.yaml',
  'multi-warning.openapi.yaml',
  'nullable-oneof.openapi.yaml',
  'nullable-optional.openapi.yaml',
  'oneof-anyof-composition.openapi.json',
  'oneof-anyof-composition.openapi.yaml',
  'petstore-minimal.openapi.json',
  'petstore-minimal.openapi.yaml',
  'petstore-rich.openapi.json',
  'petstore-rich.openapi.yaml',
  'recursive-model.openapi.yaml',
  'recursive-oneof.openapi.yaml',
  'reserved-prop-names.openapi.yaml',
  'response-204-no-content.openapi.yaml',
  'response-blob-via-pdf.openapi.yaml',
  'response-default-fallback.openapi.yaml',
  'response-octet-stream.openapi.yaml',
  'response-problem-json.openapi.yaml',
  'response-text-via-text-plain.openapi.yaml',
  'security-schemes.openapi.yaml',
  'single-entry-composition.openapi.yaml',
  'string-formats.openapi.yaml',
];

/**
 * Fixtures with no snapshot. Every entry but `malformed.yaml` is a gap to
 * close; that one's wording follows the YAML parser's own line and column
 * output, which the spec asserts by regex.
 */
const UNSNAPSHOTTED: readonly string[] = [
  'malformed.yaml',
  // TODO: these generate or fail deterministically and should be pinned.
  'bench-multi-tag.openapi.yaml',
  'consumer-forms-and-non-json.openapi.yaml',
  'cookie-param.openapi.yaml',
  'duplicate-operation-id.openapi.yaml',
  'duplicate-schema-name.openapi.yaml',
  'errors-typed.openapi.yaml',
  'missing-tag.openapi.yaml',
  'unsupported-trace.openapi.yaml',
  'verb-prefix.openapi.yaml',
  'warning-then-fatal.openapi.yaml',
];

const PINNED_FAILURE_FIXTURES: readonly string[] = [
  // One entry per diagnostic the pipeline can end on, so a renamed
  // subcode or a reject path rerouted through a different arm shows up
  // here rather than in a consumer's generated output.
  'empty-parameter.openapi.yaml',
  'inline-parameter.openapi.yaml',
  'invalid-enum-type.openapi.yaml',
  'invalid-enum-value.openapi.json',
  'unsupported-root.yaml',
  'unsupported-semantic.openapi.yaml',
  'additional-properties-boolean.openapi.yaml',
  'external-ref.openapi.yaml',
  'field-collision.openapi.yaml',
  'deep-nested-allof.openapi.yaml',
  'discriminator-missing-property.openapi.yaml',
  'discriminator-mapping-external-ref.openapi.yaml',
  'unbalanced-path-template.openapi.yaml',
  'anchor-fanout.openapi.yaml',
  'body-multi-content.openapi.yaml',
  'body-content-type-xml.openapi.yaml',
  'body-multipart-nested-object.openapi.yaml',
  'body-multipart-composed-field.openapi.yaml',
  'body-multipart-non-object.openapi.yaml',
  'body-multipart-open-schema.openapi.yaml',
  'body-urlencoded-binary-field.openapi.yaml',
  'body-urlencoded-nested-object.openapi.yaml',
];

/** Failure snapshots that need options the default run does not pass. */
const PARAMETERISED_FAILURES: readonly FailureSnapshot[] = [
  // The mapped-type validator refuses a schema the spec does not declare.
  {
    fixture: 'petstore-rich.openapi.yaml',
    snapshot: 'petstore-rich.openapi.yaml.invalid-mapped-type.failure.json',
    options: {
      mappedTypes: [{ schema: 'MissingSchema', import: '@demo/x', type: 'Missing' }],
    },
  },
];

const FAILURE_SNAPSHOTS: readonly FailureSnapshot[] = [
  ...PINNED_FAILURE_FIXTURES.map(fixture => ({
    fixture,
    snapshot: `${fixture}.failure.json`,
  })),
  ...PARAMETERISED_FAILURES,
];

/**
 * Fails when a fixture on disk is in none of the three sets, so a new
 * fixture must be classified rather than silently ignored.
 */
function assertEveryFixtureIsClassified(): void {
  const classified = new Set<string>([
    ...SUCCESS_FIXTURES,
    ...UNSNAPSHOTTED,
    ...FAILURE_SNAPSHOTS.map(entry => entry.fixture),
  ]);
  const unclassified = fs
    .readdirSync(fixturesDir)
    .filter(name => /\.(ya?ml|json)$/u.test(name))
    .filter(name => !classified.has(name));

  if (unclassified.length > 0) {
    console.error(
      `regen-snapshots: ${unclassified.length} fixture(s) are in none of ` +
        'SUCCESS_FIXTURES, FAILURE_SNAPSHOTS or UNSNAPSHOTTED:\n' +
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
for (const fixture of SUCCESS_FIXTURES) {
  let result;
  try {
    result = await run(fixture);
  } catch (error) {
    console.error(
      `FAIL: ${fixture} was expected to generate but failed with ` +
        `${asGenerateError(error, fixture).code}. Add it to FAILURE_SNAPSHOTS, or fix the fixture.`,
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

for (const { fixture, snapshot, options } of FAILURE_SNAPSHOTS) {
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
