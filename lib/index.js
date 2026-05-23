'use strict';

// Test affordance: set OPENAPI_NG_DISABLE_NATIVE_FOR_TEST=1 to prevent the
// native binding from loading. Used to verify lazy-load behaviour in CLI tests.
if (process.env.OPENAPI_NG_DISABLE_NATIVE_FOR_TEST === '1') {
  throw new Error('native binding load is disabled for this test');
}

// Node entry point. The native binding sits at ../native.js (auto-generated
// by napi-rs); this wrapper upgrades thrown errors into a real
// `GenerateError` JS class that extends `Error`, so consumers can write
// `err instanceof GenerateError`. The native binding itself only attaches
// own-properties to a plain Error — making the class subclass `Error` on
// the JS side is the simplest path that survives napi-rs's class
// registration not setting the Error prototype.
//
// The native binding is NOT a published entry point — `package.json#main`
// only exposes `lib/index.js`, so consumers can never bypass this wrapper.
//
// Shared option normalisation, validation, and URL-fetch ergonomics live
// in `lib/wrapper-core.js` so the browser/WASI entry (`browser.js`) can
// reuse them; this file is the Node-specific seam that binds them to
// `../native.js`.

const native = require('../native.js');
const { GenerateError } = require('./generate-error.js');
const { fetchInput } = require('./fetch-input.js');
const { prepareOptions, upgradeError } = require('./wrapper-core.js');

async function generate(options) {
  const prepared = await prepareOptions(options, fetchInput);
  try {
    return native.generate(prepared);
  } catch (err) {
    throw upgradeError(err);
  }
}

// Frozen runtime shape for `EmitTarget`. Mirrors the ambient const
// declared in `index.d.ts` and matches the `browser.js` entry, so the
// surface a consumer destructures is identical across both runtimes and
// no longer depends on napi-rs's string-enum machinery.
const EmitTarget = Object.freeze({
  Models: 'models',
  Angular: 'angular',
});

module.exports = {
  generate,
  GenerateError,
  EmitTarget,
};
