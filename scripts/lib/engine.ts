// Loads the generator from the local build. `lib/index.js` is CommonJS,
// so the shape check below turns a renamed or missing export into an
// error naming it, at load time.

import { createRequire } from 'node:module';

import type { GenerateError, GenerateOptions, GenerateResult } from '../../index.js';

export type { GenerateError, GenerateOptions, GenerateResult };

/** The one entry point a script needs. */
type Generate = (options: GenerateOptions) => Promise<GenerateResult>;

/** The runtime surface of `lib/index.js` these scripts rely on. */
interface Wrapper {
  readonly generate: Generate;
  readonly GenerateError: {
    isGenerateError: (value: unknown) => value is GenerateError;
  };
}

/** Fails naming the export that is missing or no longer a function. */
function loadWrapper(): Wrapper {
  const loaded: unknown = createRequire(import.meta.url)('../../lib/index.js');
  if (loaded === null || typeof loaded !== 'object') {
    throw new TypeError('lib/index.js did not export an object');
  }

  const exported: Partial<Wrapper> = loaded;
  if (typeof exported.generate !== 'function') {
    throw new TypeError('lib/index.js does not export generate()');
  }
  if (typeof exported.GenerateError?.isGenerateError !== 'function') {
    throw new TypeError('lib/index.js does not export GenerateError.isGenerateError()');
  }
  return { generate: exported.generate, GenerateError: exported.GenerateError };
}

const wrapper = loadWrapper();

export const generate: Generate = wrapper.generate;

/** Narrows a caught value to the generator's own failure type. */
export function isGenerateError(value: unknown): value is GenerateError {
  return wrapper.GenerateError.isGenerateError(value);
}
