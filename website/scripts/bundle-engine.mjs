#!/usr/bin/env node
// Pre-bundles the napi-rs WASI browser loader so the page can import it
// from /playground-engine/ without Vite processing node_modules internals.
// The engine comes from the repo checkout: `napi build --target
// wasm32-wasip1-threads` at the repo root writes the loader, worker and wasm
// next to package.json, so the playground always runs the commit it is built from.
import { build } from 'esbuild';
import fs from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const websiteRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const repoRoot = path.resolve(websiteRoot, '..');
const outDir = path.join(websiteRoot, 'public', 'playground-engine');

const ENGINE_FILES = [
  'openapi-ng.wasi-browser.js',
  'wasi-worker-browser.mjs',
  'openapi-ng.wasm32-wasi.wasm',
];
const missing = ENGINE_FILES.filter(file => !fs.existsSync(path.join(repoRoot, file)));
if (missing.length) {
  throw new Error(
    `bundle-engine: missing ${missing.join(', ')} at the repo root; run ` +
      '`bun run build --target wasm32-wasip1-threads` there first.',
  );
}

const PACKAGE_WORKER_URL =
  "new URL('@avsystem/openapi-ng-wasm32-wasi/wasi-worker-browser.mjs', import.meta.url)";
const LOCAL_WORKER_URL = "new URL('./wasi-worker-browser.mjs', import.meta.url)";

fs.rmSync(outDir, { recursive: true, force: true });
fs.mkdirSync(outDir, { recursive: true });

const loaderSource = fs.readFileSync(
  path.join(repoRoot, 'openapi-ng.wasi-browser.js'),
  'utf8',
);
// `napi build` emits the relative worker URL; `napi artifacts` rewrites it to
// the bare specifier, so accept both.
if (
  !loaderSource.includes(PACKAGE_WORKER_URL) &&
  !loaderSource.includes(LOCAL_WORKER_URL)
) {
  throw new Error(
    'bundle-engine: worker URL in the WASI loader changed; update PACKAGE_WORKER_URL',
  );
}

await build({
  stdin: {
    contents: loaderSource.replace(PACKAGE_WORKER_URL, LOCAL_WORKER_URL),
    resolveDir: repoRoot,
    sourcefile: 'openapi-ng.wasi-browser.js',
    loader: 'js',
  },
  bundle: true,
  format: 'esm',
  platform: 'browser',
  target: 'es2022',
  minify: true,
  outfile: path.join(outDir, 'openapi-ng.wasi-browser.js'),
});

await build({
  entryPoints: [path.join(repoRoot, 'wasi-worker-browser.mjs')],
  bundle: true,
  format: 'esm',
  platform: 'browser',
  target: 'es2022',
  minify: true,
  outfile: path.join(outDir, 'wasi-worker-browser.mjs'),
});

fs.copyFileSync(
  path.join(repoRoot, 'openapi-ng.wasm32-wasi.wasm'),
  path.join(outDir, 'openapi-ng.wasm32-wasi.wasm'),
);

const version =
  require(path.join(repoRoot, 'package.json')).version + (process.env.ENGINE_VERSION_SUFFIX ?? '');
fs.writeFileSync(path.join(outDir, 'version.json'), JSON.stringify({ version }));
console.log(`bundle-engine: wrote ${outDir} (v${version})`);
