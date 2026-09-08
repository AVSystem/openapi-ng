#!/usr/bin/env bun
// Refuses to publish while package.json still carries the placeholder
// version, which would claim 0.0.0 on the registry.

import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const PLACEHOLDER = '0.0.0';

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), '..');
const pkg = JSON.parse(readFileSync(join(repoRoot, 'package.json'), 'utf8')) as {
  version?: string;
};

if (pkg.version === PLACEHOLDER) {
  console.error(`Refusing to publish: package.json version is "${PLACEHOLDER}" (placeholder).`);
  console.error('Set a real version (`bun pm version <bump>`, then `napi version`).');
  process.exit(1);
}
