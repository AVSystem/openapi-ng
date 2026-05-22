'use strict';

// Browser/WASI entry point. Mirrors `lib/index.js`'s shape — applies the
// same option validation, URL-fetch ergonomics, and error upgrade — but
// binds to the wasm32-wasi binding from `@avsystem/openapi-ng-wasm32-wasip1-threads`
// instead of `../native.js`. Shared logic lives in `lib/wrapper-core.js`.
//
// The WASI binding is loaded lazily so this module stays importable when
// the optional `@avsystem/openapi-ng-wasm32-wasip1-threads` package isn't installed
// — the stub `generate` then throws an `E_UNSUPPORTED_RUNTIME`
// `GenerateError` at call time, which is what `__test__/browser.spec.ts`
// asserts.

const { GenerateError } = require('./lib/generate-error.js');
const { fetchInput } = require('./lib/fetch-input.js');
const { prepareOptions, upgradeError } = require('./lib/wrapper-core.js');

// Lazily try to load the WASI binding. The browser entry must remain
// importable even when the optional package isn't installed — see
// `__test__/browser.spec.ts`, which asserts that the throw lands at
// generate() call time, not at module load time.
let nativeBinding = null;
let nativeLoadError = null;
try {
  nativeBinding = require('@avsystem/openapi-ng-wasm32-wasip1-threads');
} catch (err) {
  nativeLoadError = err;
}

async function generate(options) {
  if (nativeBinding === null) {
    const cause = nativeLoadError?.message ?? String(nativeLoadError);
    throw new GenerateError({
      code: 'E_UNSUPPORTED_RUNTIME',
      message:
        `openapi-ng cannot run in this browser/runtime context: ${cause}. ` +
        `Ensure the '@avsystem/openapi-ng-wasm32-wasip1-threads' package is installed and reachable.`,
      warnings: [],
    });
  }
  const prepared = await prepareOptions(options, fetchInput);
  try {
    return nativeBinding.generate(prepared);
  } catch (err) {
    throw upgradeError(err);
  }
}

// Frozen runtime shape for `EmitTarget`. Mirrors the ambient const
// declared in `index.d.ts` and matches the `lib/index.js` entry, so the
// surface a consumer destructures is identical across both runtimes.
const EmitTarget = Object.freeze({
  Models: 'models',
  Angular: 'angular',
});

module.exports = {
  generate,
  GenerateError,
  EmitTarget,
};
