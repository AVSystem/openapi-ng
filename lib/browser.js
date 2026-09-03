'use strict';

// Browser entry. Options go through the same normalisation as the Node
// entry; generation runs in the WebAssembly binding published as
// `@avsystem/openapi-ng-wasm32-wasi`, whose `browser` field points at
// napi-rs's WASI browser loader. The page must be cross-origin isolated
// (COOP `same-origin`, COEP `require-corp`) because the binding uses
// shared memory.

const { GenerateError } = require('./generate-error.js');
const { prepareOptions, unwrapOutcome, upgradeError } = require('./wrapper-core.js');

const WASI_PACKAGE = '@avsystem/openapi-ng-wasm32-wasi';

function invalidOption(message) {
  return new GenerateError({
    code: 'E_INVALID_OPTION',
    subcode: 'shape',
    message,
    warnings: [],
  });
}

// No filesystem and no node:net in the browser: the spec arrives as
// `inputContents`, artifacts leave as `result.artifacts`.
function rejectPathOptions(options) {
  if (options === null || typeof options !== 'object') return;
  if (options.inputPath !== undefined) {
    throw invalidOption(
      'inputPath is not available in the browser entry; read the spec yourself and pass inputContents with displayPath.',
    );
  }
  if (options.outputPath !== undefined) {
    throw invalidOption(
      'outputPath is not available in the browser entry; write result.artifacts yourself.',
    );
  }
}

function unreachableFetch() {
  throw new Error(
    'openapi-ng: URL inputs are rejected before fetch in the browser entry',
  );
}

function unsupportedRuntime(cause) {
  const err = new GenerateError({
    code: 'E_UNSUPPORTED_RUNTIME',
    message:
      `openapi-ng could not load its WebAssembly binding (${WASI_PACKAGE}). ` +
      `Install that package next to @avsystem/openapi-ng and serve the page with ` +
      `Cross-Origin-Opener-Policy: same-origin and Cross-Origin-Embedder-Policy: require-corp. ` +
      `Cause: ${cause && cause.message ? cause.message : String(cause)}`,
    warnings: [],
  });
  err.cause = cause;
  return err;
}

// `loadBinding` resolves to the WASI module namespace (ESM loader) or a
// CommonJS `module.exports` under `default`; both expose `generateNative`.
function createGenerate(loadBinding) {
  let bindingPromise;
  const load = () => {
    if (!bindingPromise) {
      bindingPromise = loadBinding()
        .then(mod => {
          const binding =
            mod && typeof mod.generateNative === 'function' ? mod : mod && mod.default;
          if (!binding || typeof binding.generateNative !== 'function') {
            // Plain Error: the catch below turns it into E_UNSUPPORTED_RUNTIME
            // and clears the cached promise, so a bad shape and a failed load
            // surface identically.
            throw new Error('module does not export generateNative');
          }
          return binding;
        })
        .catch(cause => {
          bindingPromise = undefined;
          throw unsupportedRuntime(cause);
        });
    }
    return bindingPromise;
  };

  return async function generate(options) {
    rejectPathOptions(options);
    const prepared = await prepareOptions(options, unreachableFetch);
    const binding = await load();
    let outcome;
    try {
      outcome = binding.generateNative(prepared);
    } catch (err) {
      throw upgradeError(err);
    }
    return unwrapOutcome(outcome);
  };
}

const generate = createGenerate(() => import(WASI_PACKAGE));

// Mirrors the frozen const in `lib/index.js` and `index.d.ts`.
const EmitTarget = Object.freeze({
  Models: 'models',
  Angular: 'angular',
});

module.exports = {
  generate,
  createGenerate,
  GenerateError,
  EmitTarget,
};
