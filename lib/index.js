'use strict';

// OPENAPI_NG_DISABLE_NATIVE_FOR_TEST=1 prevents the native binding from
// loading.
if (process.env.OPENAPI_NG_DISABLE_NATIVE_FOR_TEST === '1') {
  throw new Error('native binding load is disabled for this test');
}

// Node entry point, and the only published one: it binds
// `lib/wrapper-core.js` to the napi-rs binding at `../native.js`. The
// binding returns a `{ result, error }` union and never throws; this
// wrapper raises `GenerateError` from the error arm.

/**
 * The native binding, whose only export these wrappers use.
 *
 * @type {{ generateNative: (options: unknown) => import('./wrapper-core.js').GenerateOutcome }}
 */
const native = require('../native.js');
const { GenerateError } = require('./generate-error.js');
const { fetchInput } = require('./fetch-input.js');
const { prepareOptions, unwrapOutcome, upgradeError } = require('./wrapper-core.js');

/**
 * @param {import('../index.js').GenerateOptions} options
 * @returns {Promise<import('../index.js').GenerateResult>}
 */
async function generate(options) {
  const prepared = await prepareOptions(options, fetchInput);
  let outcome;
  try {
    outcome = native.generateNative(prepared);
  } catch (err) {
    throw upgradeError(err);
  }
  return unwrapOutcome(outcome);
}

// Runtime shape for `EmitTarget`, mirroring `index.d.ts` and the
// `browser.js` entry.
const EmitTarget = Object.freeze({
  Models: 'models',
  Angular: 'angular',
});

module.exports = {
  generate,
  GenerateError,
  EmitTarget,
};
