import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { resolve, dirname } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const pkg = JSON.parse(readFileSync(resolve(here, '..', 'package.json'), 'utf8'));

if (pkg.version === '0.0.0') {
  console.error(`Refusing to publish: package.json version is "0.0.0" (placeholder).`);
  console.error(
    `Set a real version (e.g. via \`pnpm version <bump>\` then \`napi version\`).`,
  );
  process.exit(1);
}
