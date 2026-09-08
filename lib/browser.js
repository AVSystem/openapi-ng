'use strict';

// Browser entry: the same option normalisation as the Node entry, over
// the WebAssembly binding in `@avsystem/openapi-ng-wasm32-wasi`. The
// binding uses shared memory, so the page must be cross-origin isolated
// (COOP `same-origin`, COEP `require-corp`).

const { GenerateError } = require('./generate-error.js');
const { prepareOptions, unwrapOutcome, upgradeError } = require('./wrapper-core.js');
const { field } = require('./diagnostic.js');

const WASI_PACKAGE = '@avsystem/openapi-ng-wasm32-wasi';

/**
 * The WebAssembly binding's only export these wrappers use.
 *
 * @typedef {{ generateNative: (options: unknown) =>
 *   import('./wrapper-core.js').GenerateOutcome }} WasiBinding
 */

/** @param {unknown} value @returns {value is WasiBinding} */
function isWasiBinding(value) {
  return (
    typeof value === 'object' &&
    value !== null &&
    typeof (/** @type {{ generateNative?: unknown }} */ (value).generateNative) === 'function'
  );
}

/**
 * @param {string} message
 * @returns {import('../index.js').GenerateError}
 */
function invalidOption(message) {
  return new GenerateError({
    code: 'E_INVALID_OPTION',
    subcode: 'shape',
    message,
    warnings: [],
  });
}

// No filesystem in the browser: the spec arrives as `inputContents` and
// artifacts leave as `result.artifacts`.
/**
 * @param {import('../index.js').GenerateOptions} options
 * @returns {void}
 */
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

/** @returns {never} */
function unreachableFetch() {
  throw new Error(
    'openapi-ng: URL inputs are rejected before fetch in the browser entry',
  );
}

/**
 * @param {unknown} cause
 * @returns {import('../index.js').GenerateError}
 */
function unsupportedRuntime(cause) {
  const message = field(cause, 'message');
  const reason = message ? String(message) : String(cause);
  const err = new GenerateError({
    code: 'E_UNSUPPORTED_RUNTIME',
    message:
      `openapi-ng could not load its WebAssembly binding (${WASI_PACKAGE}). ` +
      `Install that package next to @avsystem/openapi-ng and serve the page with ` +
      `Cross-Origin-Opener-Policy: same-origin and Cross-Origin-Embedder-Policy: require-corp. ` +
      `Cause: ${reason}`,
    warnings: [],
  });
  err.cause = cause;
  return err;
}

/**
 * @param {() => Promise<unknown>} loadBinding Resolves to the WASI module
 *   namespace, or to a CommonJS `module.exports` under `default`.
 * @returns {(options: import('../index.js').GenerateOptions) =>
 *   Promise<import('../index.js').GenerateResult>}
 */
function createGenerate(loadBinding) {
  /** @type {Promise<WasiBinding> | undefined} */
  let bindingPromise;
  const load = () => {
    if (!bindingPromise) {
      bindingPromise = loadBinding()
        .then(mod => {
          const namespace = /** @type {{ default?: unknown }} */ (mod);
          const binding = isWasiBinding(mod) ? mod : namespace?.default;
          if (!isWasiBinding(binding)) {
            // The catch below turns this into the E_UNSUPPORTED_RUNTIME a
            // failed load produces.
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

  /**
   * @param {import('../index.js').GenerateOptions} options
   * @returns {Promise<import('../index.js').GenerateResult>}
   */
  return async function generate(options) {
    rejectPathOptions(options);
    const [prepared, binding] = await Promise.all([
      prepareOptions(options, unreachableFetch),
      load(),
    ]);
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
