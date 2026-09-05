#!/usr/bin/env node
// Pre-bundles the napi-rs WASI browser loader so the page can import it
// from /playground-engine/ without Vite processing node_modules internals.
import { build } from 'esbuild';
import fs from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const websiteRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const outDir = path.join(websiteRoot, 'public', 'playground-engine');
const pkgDir = path.dirname(
  require.resolve('@avsystem/openapi-ng-wasm32-wasi/package.json'),
);

const PACKAGE_WORKER_URL =
  "new URL('@avsystem/openapi-ng-wasm32-wasi/wasi-worker-browser.mjs', import.meta.url)";
const LOCAL_WORKER_URL = "new URL('./wasi-worker-browser.mjs', import.meta.url)";

fs.rmSync(outDir, { recursive: true, force: true });
fs.mkdirSync(outDir, { recursive: true });

const loaderSource = fs.readFileSync(
  path.join(pkgDir, 'openapi-ng.wasi-browser.js'),
  'utf8',
);
// `napi artifacts` rewrites the published loader to the bare specifier; a local
// `napi build` already emits the relative one, so the replace below is a no-op.
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
    resolveDir: pkgDir,
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
  entryPoints: [path.join(pkgDir, 'wasi-worker-browser.mjs')],
  bundle: true,
  format: 'esm',
  platform: 'browser',
  target: 'es2022',
  minify: true,
  outfile: path.join(outDir, 'wasi-worker-browser.mjs'),
});

fs.copyFileSync(
  path.join(pkgDir, 'openapi-ng.wasm32-wasi.wasm'),
  path.join(outDir, 'openapi-ng.wasm32-wasi.wasm'),
);

const { version } = require('@avsystem/openapi-ng-wasm32-wasi/package.json');
fs.writeFileSync(path.join(outDir, 'version.json'), JSON.stringify({ version }));
console.log(`bundle-engine: wrote ${outDir} (v${version})`);
