// Smoke test for the WASI/WASM build of openapi-ng.
//
// This script forces the WASI binding path in `native.js` by setting
// `NAPI_RS_FORCE_WASI=1` before requiring the library. It is intended to be
// run after `pnpm build --target wasm32-wasip1-threads` has produced the
// `openapi-ng.wasm32-wasi.wasm` artifact (and the matching `openapi-ng.wasi.cjs`
// loader) in the project root.
//
// Usage:
//   NAPI_RS_FORCE_WASI=1 node scripts/smoke-wasm.mjs
//
// or simply:
//   node scripts/smoke-wasm.mjs
// (this script sets the env var itself if not already set, before importing
// the library).
//
// Exits non-zero on failure; prints a one-line success message otherwise so
// the CI matrix leg (T6.2) can assert on stdout.
//
// WARNING: `pnpm build --target wasm32-wasip1-threads` rewrites `browser.js`
// to an auto-generated `export * from '...'` stub, overwriting the
// hand-written `E_UNSUPPORTED_RUNTIME` diagnostic. After running a WASI
// build, verify with `git diff browser.js` and restore if needed
// (`git restore browser.js`). T6.3 reworks the browser-runtime diagnostic;
// T6.2 wires CI to either suppress this rewrite or restore the hand-written
// file post-build.

import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

// Force WASI before the native binding is loaded.
if (!process.env.NAPI_RS_FORCE_WASI) {
  process.env.NAPI_RS_FORCE_WASI = '1';
}

const require = createRequire(import.meta.url);
const __dirname = dirname(fileURLToPath(import.meta.url));
const projectRoot = resolve(__dirname, '..');

const { generate } = require(resolve(projectRoot, 'lib/index.js'));

const inputPath = resolve(projectRoot, 'test/fixtures/petstore-minimal.openapi.yaml');

const result = generate({
  inputPath,
  emit: ['models'],
});

if (!result || !Array.isArray(result.artifacts) || result.artifacts.length === 0) {
  console.error('WASM smoke FAILED: no artifacts produced');
  process.exit(1);
}

console.log(`WASM smoke OK; artifacts: ${result.artifacts.length}`);
