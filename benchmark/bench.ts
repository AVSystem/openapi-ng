import { Bench } from 'tinybench';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import openapiNg from '../lib/index.js';

const { generate } = openapiNg;

const __dirname = dirname(fileURLToPath(import.meta.url));

const outputPath = mkdtempSync(join(tmpdir(), 'openapi-ng-bench-'));

/**
 * Synthesise a stress spec with `count` resources, each carrying its own
 * Resource/ResourceList/ResourceStatus triple (3*count schemas) and a
 * GET-list + POST-create pair (2*count operations). Written to a temp
 * dir and pointed at by the runner; not committed because the file
 * would be ~10× the size of bench-large.yaml without exercising any
 * code path that bench-large doesn't already cover.
 *
 * Regression smoke test only: kept under bench-budgets.json with
 * generous headroom (the goal is "still finishes", not a tight perf
 * gate). bench-large remains the load-bearing scale benchmark.
 */
function writeStressSpec(resourceCount: number): string {
  const lines: string[] = [
    'openapi: 3.0.3',
    'info:',
    '  title: Bench Stress',
    '  version: 1.0.0',
    'paths:',
  ];
  for (let i = 1; i <= resourceCount; i++) {
    lines.push(`  /resource${i}:`);
    lines.push('    get:');
    lines.push(`      operationId: getResource${i}`);
    lines.push(`      tags: [Resource${i}]`);
    lines.push('      responses:');
    lines.push(`        '200':`);
    lines.push(`          description: ok`);
    lines.push('          content:');
    lines.push('            application/json:');
    lines.push('              schema:');
    lines.push(`                $ref: '#/components/schemas/Resource${i}List'`);
    lines.push('    post:');
    lines.push(`      operationId: createResource${i}`);
    lines.push(`      tags: [Resource${i}]`);
    lines.push('      requestBody:');
    lines.push('        required: true');
    lines.push('        content:');
    lines.push('          application/json:');
    lines.push('            schema:');
    lines.push(`              $ref: '#/components/schemas/Resource${i}'`);
    lines.push('      responses:');
    lines.push(`        '200':`);
    lines.push(`          description: ok`);
    lines.push('          content:');
    lines.push('            application/json:');
    lines.push('              schema:');
    lines.push(`                $ref: '#/components/schemas/Resource${i}'`);
  }
  lines.push('components:');
  lines.push('  schemas:');
  for (let i = 1; i <= resourceCount; i++) {
    lines.push(`    Resource${i}:`);
    lines.push('      type: object');
    lines.push('      required: [id, status]');
    lines.push('      properties:');
    lines.push('        id:');
    lines.push('          type: string');
    lines.push('        status:');
    lines.push(`          $ref: '#/components/schemas/Resource${i}Status'`);
    lines.push(`    Resource${i}Status:`);
    lines.push('      type: string');
    lines.push('      enum: [active, inactive]');
    lines.push(`    Resource${i}List:`);
    lines.push('      type: object');
    lines.push('      required: [items]');
    lines.push('      properties:');
    lines.push('        items:');
    lines.push('          type: array');
    lines.push('          items:');
    lines.push(`            $ref: '#/components/schemas/Resource${i}'`);
  }
  const target = join(outputPath, 'bench-stress.openapi.yaml');
  writeFileSync(target, lines.join('\n') + '\n', 'utf8');
  return target;
}

// 250 resources × (3 schemas + 2 ops) = 750 schemas + 500 ops. Chosen
// to land squarely above bench-large (90 schemas / 60 ops) without
// committing a 20k-line YAML. Tweakable via the constant below.
const STRESS_RESOURCE_COUNT = 250;
const stressSpecPath = writeStressSpec(STRESS_RESOURCE_COUNT);

const bench = new Bench({ warmupIterations: 100 });

bench.add('generate (petstore-rich, yaml)', async () => {
  await generate({
    inputPath: 'test/fixtures/petstore-rich.openapi.yaml',
    outputPath,
    emit: ['models', 'angular'],
  });
});

bench.add('generate (petstore-rich, json)', async () => {
  await generate({
    inputPath: 'test/fixtures/petstore-rich.openapi.json',
    outputPath,
    emit: ['models', 'angular'],
  });
});

bench.add('generate (petstore-minimal, yaml)', async () => {
  await generate({
    inputPath: 'test/fixtures/petstore-minimal.openapi.yaml',
    outputPath,
    emit: ['models', 'angular'],
  });
});

// E11: large-spec benchmark — 30 paths × 2 ops = 60 operations and 90 schemas
// (30 entities × {Resource, ResourceStatus, ResourceList}), grouped under
// 6 tags (Resource1..Resource6, 10 ops each). Synthetic but shaped like a
// real REST API; useful for catching phase-level regressions
// (normalize/lower/emit) that don't surface in petstore-sized inputs.
bench.add('generate (bench-large, yaml)', async () => {
  await generate({
    inputPath: 'test/fixtures/bench-large.openapi.yaml',
    outputPath,
    emit: ['models', 'angular'],
  });
});

// bench-multi-tag exercises by-tag grouping at a different scale: 5 tags ×
// 20 ops each (100 ops total). Complements bench-large by stressing
// operation_grouper's per-group emission path with denser group fanout.
bench.add('generate (bench-multi-tag, yaml)', async () => {
  await generate({
    inputPath: 'test/fixtures/bench-multi-tag.openapi.yaml',
    outputPath,
    emit: ['models', 'angular'],
  });
});

// bench-stress: synthesised at startup, ~750 schemas + 500 operations.
// Pure regression smoke test for perf — exercises the same code paths
// as bench-large but at ~10× the schema count and ~8× the operation
// count. Budget intentionally generous (the goal is "still finishes",
// not a tight per-ns gate); bench-large remains the load-bearing
// scale benchmark for tight regression detection.
bench.add('generate (bench-stress, yaml)', async () => {
  await generate({
    inputPath: stressSpecPath,
    outputPath,
    emit: ['models', 'angular'],
  });
});

await bench.run();

console.table(bench.table());

// Perf budgets live in bench-budgets.json (mean latency in milliseconds);
// the runner fails when a task's measured mean exceeds budget × 1.25. The
// 25% headroom absorbs noise and slower CI runners while still catching
// real regressions. Tasks without a budget entry are exempt.
// `tinybench`'s `result.latency.mean` is already in milliseconds; the
// table column header reads "ns" because the display layer multiplies by
// 1e6 before formatting.
const BUDGET_HEADROOM = 1.25;
const budgetsPath = join(__dirname, 'bench-budgets.json');
const { budgets }: { budgets: Record<string, number> } = JSON.parse(
  readFileSync(budgetsPath, 'utf8'),
);

const regressions: string[] = [];
for (const task of bench.tasks) {
  const budgetMs = budgets[task.name];
  if (budgetMs === undefined) continue;
  const result = task.result;
  if (!result) continue;
  const meanMs = result.latency.mean;
  const limitMs = budgetMs * BUDGET_HEADROOM;
  if (meanMs > limitMs) {
    regressions.push(
      `  - ${task.name}: mean ${meanMs.toFixed(3)}ms exceeds budget ${budgetMs}ms × ${BUDGET_HEADROOM} (${limitMs.toFixed(3)}ms)`,
    );
  }
}
if (regressions.length > 0) {
  console.error(`Perf regression detected:\n${regressions.join('\n')}`);
  process.exit(1);
}
