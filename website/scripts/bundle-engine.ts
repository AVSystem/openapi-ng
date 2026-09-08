#!/usr/bin/env bun
// Pre-bundles the napi-rs WASI browser loader into public/playground-engine/
// so the playground page can import it without Vite reaching into
// node_modules internals.

import fs from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { build } from 'esbuild';

const WASI_PACKAGE = '@avsystem/openapi-ng-wasm32-wasi';

/** The worker specifier `napi artifacts` rewrites into the published loader. */
const PACKAGE_WORKER_URL = `new URL('${WASI_PACKAGE}/wasi-worker-browser.mjs', import.meta.url)`;
/** The specifier a local `napi build` emits instead. */
const LOCAL_WORKER_URL = "new URL('./wasi-worker-browser.mjs', import.meta.url)";

const require = createRequire(import.meta.url);
const websiteRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const outDir = path.join(websiteRoot, 'public', 'playground-engine');
const packageDir = path.dirname(require.resolve(`${WASI_PACKAGE}/package.json`));

const shared = {
  bundle: true,
  format: 'esm',
  platform: 'browser',
  target: 'es2022',
  minify: true,
} as const;

/**
 * Rewrites the loader's worker URL to the local one so both a published and
 * a locally built engine resolve the worker from the output directory.
 */
function localiseWorkerUrl(source: string): string {
  if (!source.includes(PACKAGE_WORKER_URL) && !source.includes(LOCAL_WORKER_URL)) {
    throw new Error(
      'bundle-engine: the worker URL in the WASI loader changed; update PACKAGE_WORKER_URL',
    );
  }
  return source.replace(PACKAGE_WORKER_URL, LOCAL_WORKER_URL);
}

fs.rmSync(outDir, { recursive: true, force: true });
fs.mkdirSync(outDir, { recursive: true });

const loaderName = 'openapi-ng.wasi-browser.js';
const loaderSource = fs.readFileSync(path.join(packageDir, loaderName), 'utf8');

await build({
  ...shared,
  stdin: {
    contents: localiseWorkerUrl(loaderSource),
    resolveDir: packageDir,
    sourcefile: loaderName,
    loader: 'js',
  },
  outfile: path.join(outDir, loaderName),
});

await build({
  ...shared,
  entryPoints: [path.join(packageDir, 'wasi-worker-browser.mjs')],
  outfile: path.join(outDir, 'wasi-worker-browser.mjs'),
});

fs.copyFileSync(
  path.join(packageDir, 'openapi-ng.wasm32-wasi.wasm'),
  path.join(outDir, 'openapi-ng.wasm32-wasi.wasm'),
);

const { version } = require(`${WASI_PACKAGE}/package.json`) as { version: string };
fs.writeFileSync(path.join(outDir, 'version.json'), JSON.stringify({ version }));
console.log(`bundle-engine: wrote ${outDir} (v${version})`);
