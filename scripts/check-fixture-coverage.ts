#!/usr/bin/env bun
// Fails when a fixture in test/fixtures/ appears in none of the three sets
// in scripts/lib/snapshot-layout.ts. Needs no built binding.

import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { unclassifiedFixtures } from './lib/snapshot-layout.ts';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const unclassified = unclassifiedFixtures(repoRoot);

if (unclassified.length > 0) {
  console.error(
    `${unclassified.length} fixture(s) appear in none of SUCCESS_FIXTURES, ` +
      'FAILURE_FIXTURES or UNSNAPSHOTTED:\n' +
      unclassified.map(name => `  ${name}`).join('\n'),
  );
  process.exit(1);
}

console.log(`fixture coverage ok: every fixture in ${path.join('test', 'fixtures')} is classified`);
