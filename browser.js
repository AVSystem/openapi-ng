'use strict';

// Browser/edge entry point. openapi-ng requires the native binding to
// generate code, so any browser or edge runtime (Vite/Webpack/esbuild
// resolving the `browser` field, Cloudflare Workers, Vercel Edge, etc.)
// gets a stub that throws `E_UNSUPPORTED_RUNTIME` at call time. The
// module itself stays importable so bundlers don't choke at build time.

const { GenerateError } = require('./lib/generate-error.js');

async function generate() {
  throw new GenerateError({
    code: 'E_UNSUPPORTED_RUNTIME',
    message:
      'openapi-ng does not support browser or edge runtimes. ' +
      'Run the generator from Node, or remove openapi-ng from your browser bundle.',
    warnings: [],
  });
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
