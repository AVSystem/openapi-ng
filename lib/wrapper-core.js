'use strict';

// Shared wrapper logic for the Node entry (`lib/index.js`) and the
// browser/WASI entry (`browser.js`). Both entries normalise + validate
// options the same way and reuse the same URL-fetch ergonomics; only the
// concrete binding (`require('../native.js')` vs
// `require('@avsystem/openapi-ng-wasm32-wasi')`) differs. Keeping the shared
// surface in one place is what makes that promise true.

const { GenerateError } = require('./generate-error.js');

// Frozen allow-list of recognised option keys. The native binding silently
// ignores anything else; surfacing unknown keys here means typos
// (`inputpath:` → undefined) fail fast with a typed `GenerateError`
// instead of producing confusing downstream diagnostics. Keep in sync
// with `GenerateOptions` in `src/bindings.rs`.
const GENERATE_OPTION_KEYS = Object.freeze(
  new Set([
    'inputPath',
    'inputContents',
    'displayPath',
    'inputFormat',
    'outputPath',
    'emit',
    'mappedTypes',
    'responseTypeMapping',
    'naming',
  ]),
);

// Frozen allow-list of recognised `EmitTarget` runtime values. Duplicates
// `VALID_EMIT_TARGETS` in `bin/lib/parse.js` on purpose: the CLI and the
// programmatic API are independent boundaries, each performing its own
// entry-level validation. Both reflect the same truth declared in
// `EmitTarget` (see `index.d.ts`); keep them in sync.
const VALID_EMIT = Object.freeze(new Set(['models', 'angular']));

// The five case transformations supported by naming rules (see
// `docs/naming-spec.md`). Mirrored on the Rust side; both must accept
// the same lowercase strings.
const VALID_CASES = Object.freeze(
  new Set(['camel', 'pascal', 'snake', 'kebab', 'constant']),
);

// Default emit set. Mirrors `DEFAULT_EMIT` in `bin/lib/parse.js` so the
// programmatic API matches the CLI: a consumer who calls `generate({
// inputPath })` without an explicit `emit` list gets the same artifacts as
// `openapi-ng generate -i ...`. Frozen so a consumer mutating the
// returned options object can't corrupt the next call's default.
const DEFAULT_EMIT = Object.freeze(['models', 'angular']);

// Lower one chain item (either a string or a Rule-shaped object) into
// the `{ string }` or `{ rule }` shape the Rust side expects.
function normalizeNamingEntry(entry, path) {
  if (typeof entry === 'string') {
    return { string: entry };
  }
  if (entry === null || typeof entry !== 'object' || Array.isArray(entry)) {
    throw new GenerateError({
      code: 'E_INVALID_OPTION',
      subcode: 'shape',
      message: `naming.${path}: each entry must be a string or a Rule object.`,
      warnings: [],
    });
  }
  if (entry.case !== undefined && !VALID_CASES.has(entry.case)) {
    throw new GenerateError({
      code: 'E_INVALID_OPTION',
      subcode: 'shape',
      message: `naming.${path}.case: '${entry.case}' is not one of 'camel', 'pascal', 'snake', 'kebab', 'constant'.`,
      warnings: [],
    });
  }
  let parse;
  if (entry.parse !== undefined) {
    if (entry.parse instanceof RegExp) {
      parse = { source: entry.parse.source, flags: entry.parse.flags };
    } else if (
      entry.parse &&
      typeof entry.parse.source === 'string' &&
      typeof entry.parse.flags === 'string'
    ) {
      parse = { source: entry.parse.source, flags: entry.parse.flags };
    } else {
      throw new GenerateError({
        code: 'E_INVALID_OPTION',
        subcode: 'shape',
        message: `naming.${path}.parse: must be a RegExp or {source, flags} object.`,
        warnings: [],
      });
    }
  }
  return {
    rule: {
      from: entry.from,
      parse,
      format: entry.format,
      case: entry.case,
    },
  };
}

// Lower the top-level `methodName` or `group` value into the NamingValue
// shape: { string } | { rule } | { chain: [...] }. Returns undefined
// when the input is undefined (keeps the option absent on the Rust side).
function normalizeNamingValue(value, key) {
  if (value === undefined) return undefined;
  if (typeof value === 'string') {
    return { string: value };
  }
  if (Array.isArray(value)) {
    return {
      chain: value.map((item, i) => normalizeNamingEntry(item, `${key}[${i}]`)),
    };
  }
  // Single rule object: lower it as a chain item, then promote to a
  // top-level `{ rule }` value.
  const entry = normalizeNamingEntry(value, key);
  return { rule: entry.rule };
}

function upgradeError(err) {
  if (GenerateError.isGenerateError(err)) {
    const upgraded = new GenerateError({
      code: err.code,
      subcode: err.subcode,
      message: err.message,
      path: err.path,
      warnings: err.warnings,
    });
    if (typeof err.stack === 'string') upgraded.stack = err.stack;
    return upgraded;
  }
  return err;
}

function validateGenerateOptions(options) {
  if (options === null || typeof options !== 'object') {
    return;
  }
  const unknown = [];
  for (const key of Object.keys(options)) {
    if (!GENERATE_OPTION_KEYS.has(key)) unknown.push(key);
  }
  if (unknown.length > 0) {
    throw new GenerateError({
      code: 'E_INVALID_OPTION',
      message:
        `Unknown generate option(s): ${unknown.map(k => `'${k}'`).join(', ')}. ` +
        `Allowed: ${[...GENERATE_OPTION_KEYS].map(k => `'${k}'`).join(', ')}.`,
      warnings: [],
    });
  }
  // Shape checks for the typed fields. NAPI rejects a wrong type on the
  // Rust side, but the error there is generic ("Failed to convert");
  // catching the mistake here gives the consumer a typed GenerateError
  // with a message that names the offending option and (where helpful)
  // the offending value.
  const hasInputPath = typeof options.inputPath === 'string' && options.inputPath !== '';
  const hasInputContents = typeof options.inputContents === 'string';

  if (hasInputPath === hasInputContents) {
    throw new GenerateError({
      code: 'E_INVALID_OPTION',
      subcode: 'shape',
      message:
        'Must set exactly one of inputPath (non-empty string) or inputContents (string).',
      warnings: [],
    });
  }

  if (hasInputContents) {
    if (typeof options.displayPath !== 'string' || options.displayPath === '') {
      throw new GenerateError({
        code: 'E_INVALID_OPTION',
        subcode: 'shape',
        message: 'displayPath (non-empty string) is required when inputContents is set.',
        warnings: [],
      });
    }
  } else if (options.displayPath !== undefined) {
    // displayPath with inputPath is ignored on the Rust side, but accepting
    // it silently would let a misconfigured caller think it's being used.
    // Reject loudly.
    throw new GenerateError({
      code: 'E_INVALID_OPTION',
      subcode: 'shape',
      message:
        'displayPath is only used with inputContents; remove it when passing inputPath.',
      warnings: [],
    });
  }

  if (options.inputFormat !== undefined) {
    if (!hasInputContents) {
      throw new GenerateError({
        code: 'E_INVALID_OPTION',
        subcode: 'shape',
        message: 'inputFormat is only honoured with inputContents.',
        warnings: [],
      });
    }
    if (options.inputFormat !== 'json' && options.inputFormat !== 'yaml') {
      throw new GenerateError({
        code: 'E_INVALID_OPTION',
        subcode: 'shape',
        message: `inputFormat must be 'json' or 'yaml'; got '${options.inputFormat}'.`,
        warnings: [],
      });
    }
  }
  if (!Array.isArray(options.emit)) {
    throw new GenerateError({
      code: 'E_INVALID_OPTION',
      subcode: 'shape',
      message: 'emit must be an array of EmitTarget',
      warnings: [],
    });
  }
  for (const target of options.emit) {
    if (!VALID_EMIT.has(target)) {
      throw new GenerateError({
        code: 'E_INVALID_OPTION',
        subcode: 'shape',
        message: `emit contains invalid target '${target}'. Allowed: 'models', 'angular'.`,
        warnings: [],
      });
    }
  }
  if (options.mappedTypes !== undefined) {
    if (!Array.isArray(options.mappedTypes)) {
      throw new GenerateError({
        code: 'E_INVALID_OPTION',
        subcode: 'shape',
        message: 'mappedTypes must be an array',
        warnings: [],
      });
    }
    for (let i = 0; i < options.mappedTypes.length; i++) {
      const mt = options.mappedTypes[i];
      if (
        typeof mt?.schema !== 'string' ||
        typeof mt?.import !== 'string' ||
        typeof mt?.type !== 'string'
      ) {
        throw new GenerateError({
          code: 'E_INVALID_OPTION',
          subcode: 'shape',
          message: `mappedTypes[${i}] must have string 'schema', 'import', and 'type' fields`,
          warnings: [],
        });
      }
    }
  }
  if (options.naming !== undefined) {
    if (
      options.naming === null ||
      typeof options.naming !== 'object' ||
      Array.isArray(options.naming)
    ) {
      throw new GenerateError({
        code: 'E_INVALID_OPTION',
        subcode: 'shape',
        message: 'naming must be an object with optional `methodName` and `group` keys',
        warnings: [],
      });
    }
  }
}

// Compute the lowered `naming` value the binding expects, without
// touching the caller's object. Returns undefined when `options.naming`
// is absent, so the caller can omit the key entirely.
function normalizeNaming(options) {
  if (options == null || typeof options !== 'object') return undefined;
  if (options.naming === undefined) return undefined;
  return {
    methodName: normalizeNamingValue(options.naming.methodName, 'methodName'),
    group: normalizeNamingValue(options.naming.group, 'group'),
  };
}

// Normalise + validate options for both entries. Applies the CLI-parity
// emit default, transparently fetches https URLs into inputContents, and
// runs the shape validator. `fetchInputFn` is injected so the browser
// entry can pass its own fetch implementation if needed; the Node entry
// passes `lib/fetch-input.js`'s `fetchInput`.
async function prepareOptions(options, fetchInputFn) {
  // Apply the CLI-parity default BEFORE validation so the validator and
  // the binding boundary both see a populated emit set. If `options` is
  // anything other than an object, leave it untouched and let
  // `validateGenerateOptions` (a no-op for non-objects) defer to the
  // binding's own type rejection.
  const normalized =
    options != null && typeof options === 'object' && options.emit === undefined
      ? { ...options, emit: [...DEFAULT_EMIT] }
      : options;

  // URL branch: if inputPath is a URL string, fetch and rewrite into
  // the inputContents form before validation. The wrapper-side
  // validator treats inputContents as the operative input from here on.
  // The http:// case is rejected inside fetchInput's scheme check —
  // duplicating the check here would just split the error message
  // surface across two layers.
  if (
    normalized != null &&
    typeof normalized === 'object' &&
    typeof normalized.inputPath === 'string' &&
    /^https?:\/\//i.test(normalized.inputPath) &&
    normalized.inputContents === undefined
  ) {
    const url = normalized.inputPath;
    const fetched = await fetchInputFn(url);
    const rewritten = { ...normalized };
    delete rewritten.inputPath;
    rewritten.inputContents = fetched.contents;
    // Use the original URL (not finalUrl after redirects) so banners
    // show what the consumer actually asked for.
    rewritten.displayPath = url;
    if (fetched.format !== null) {
      rewritten.inputFormat = fetched.format;
    }
    validateGenerateOptions(rewritten);
    const naming = normalizeNaming(rewritten);
    return naming === undefined ? rewritten : { ...rewritten, naming };
  }
  validateGenerateOptions(normalized);
  const naming = normalizeNaming(normalized);
  return naming === undefined ? normalized : { ...normalized, naming };
}

module.exports = {
  GENERATE_OPTION_KEYS,
  VALID_EMIT,
  VALID_CASES,
  DEFAULT_EMIT,
  normalizeNamingEntry,
  normalizeNamingValue,
  validateGenerateOptions,
  upgradeError,
  prepareOptions,
};
