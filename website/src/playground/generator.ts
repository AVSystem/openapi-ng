import { createGenerate, GenerateError } from '@avsystem/openapi-ng/browser';
import type { GenerateOptions, GenerateResult } from '@avsystem/openapi-ng/browser';

export type GenerateFn = (options: GenerateOptions) => Promise<GenerateResult>;
export { GenerateError };

// Pre-bundled by scripts/bundle-engine.mjs; kept out of Vite's graph so the
// napi-rs loader's worker and wasm URLs resolve relative to this file.
const ENGINE_URL = '/playground-engine/openapi-ng.wasi-browser.js';

export function loadGenerate(): GenerateFn {
  // An absolute URL keeps Vite's dev client from appending `?import`, which
  // would route the request through its transform pipeline and reject a public/ file.
  const url = new URL(ENGINE_URL, location.origin).href;
  return createGenerate(() => import(/* @vite-ignore */ url));
}

export async function engineVersion(): Promise<string> {
  const response = await fetch('/playground-engine/version.json');
  if (!response.ok) throw new Error(`engine version lookup failed (${response.status})`);
  const { version } = (await response.json()) as { version: string };
  return version;
}
