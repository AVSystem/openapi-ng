'use strict';

// Wrapper logic for the Node entry (`lib/index.js`): option normalisation
// and validation, URL-fetch ergonomics, and error upgrade around the native
// binding. Lives here rather than inline in `lib/index.js` to keep the entry
// thin and to make the pre-NAPI option surface independently testable.

const { GenerateError } = require('./generate-error.js');
const { field } = require('./diagnostic.js');

/**
 * What the native export returns: exactly one field is set. Not part of
 * the published surface, so it is declared here rather than imported.
 *
 * @typedef {object} GenerateOutcome
 * @property {import('../index.js').GenerateResult} [result]
 * @property {import('../index.js').GenerateErrorPayload} [error]
 */

/** @typedef {import('../index.js').GenerateOptions} GenerateOptions */

/**
 * A chain item as a caller may write it.
 *
 * Wider than the published `NamingRule` in one place: `parse` also
 * accepts an already-split `{ source, flags }`, which is what reaches
 * here when a caller unpacked the RegExp itself.
 *
 * @typedef {object} NamingEntryInput
 * @property {string} [from]
 * @property {RegExp | { source: string, flags: string }} [parse]
 * @property {string} [format]
 * @property {import('../index.js').Case} [case]
 */

/** @typedef {string | NamingEntryInput} NamingItemInput */

/**
 * Options after `prepareOptions`. `naming` is the lowered boundary shape
 * once a caller supplied one; the declared shape stays in the union
 * because an absent key keeps its input type.
 *
 * @typedef {Omit<GenerateOptions, 'naming'> & {
 *   naming?: import('../index.js').NamingOptions | import('../index.js').NamingConfig
 * }} PreparedOptions
 */

// Frozen allow-list of recognised option keys. The native binding silently
// ignores anything else; surfacing unknown keys here means typos
// (`inputpath:` → undefined) fail fast with a typed `GenerateError`
// instead of producing confusing downstream diagnostics. Keep in sync
// with `GenerateOptions` in `src/bindings.rs`.
/** @type {ReadonlySet<string>} */
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
/** @type {ReadonlySet<string>} */
const VALID_EMIT = Object.freeze(new Set(['models', 'angular']));

// The five case transformations supported by naming rules (see
// `docs/naming-spec.md`). Mirrored on the Rust side; both must accept
// the same lowercase strings.
/** @type {ReadonlySet<string>} */
const VALID_CASES = Object.freeze(
  new Set(['camel', 'pascal', 'snake', 'kebab', 'constant']),
);

// Default emit set. Mirrors `DEFAULT_EMIT` in `bin/lib/parse.js` so the
// programmatic API matches the CLI: a consumer who calls `generate({
// inputPath })` without an explicit `emit` list gets the same artifacts as
// `openapi-ng generate -i ...`. Frozen so a consumer mutating the
// returned options object can't corrupt the next call's default.
/** @type {readonly import('../index.js').EmitTarget[]} */
const DEFAULT_EMIT = Object.freeze(['models', 'angular']);

/**
 * Lowers one chain item into the exclusive `{ string }` or `{ rule }`
 * shape the boundary carries.
 *
 * The runtime guards stay for a JS caller who ignores the declared type.
 *
 * @param {NamingItemInput} entry
 * @param {string} path Config path, for the failure message.
 * @returns {import('../index.js').NamingChainItem}
 */
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

/**
 * Lowers one naming key's value. Returns `undefined` for an absent value,
 * which keeps the option absent at the boundary.
 *
 * @param {NamingItemInput | NamingItemInput[] | undefined} value
 * @param {string} key
 * @returns {import('../index.js').NamingValue | undefined}
 */
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
  // A single rule lowers as a chain item, then promotes to `{ rule }`.
  const entry = normalizeNamingEntry(value, key);
  return { rule: entry.rule };
}

/**
 * Turns the native union into return-or-throw.
 *
 * @param {GenerateOutcome | null | undefined} outcome
 * @returns {import('../index.js').GenerateResult}
 */
function unwrapOutcome(outcome) {
  if (outcome && outcome.error) {
    throw new GenerateError(outcome.error);
  }
  if (!outcome || !outcome.result) {
    throw new GenerateError({
      code: 'E_UNEXPECTED',
      message: 'openapi-ng: native binding returned neither a result nor an error.',
      warnings: [],
    });
  }
  return outcome.result;
}

/**
 * Wraps anything else thrown across the binding boundary — a loader
 * failure, an argument-marshalling error. A typed failure passes through.
 *
 * @param {unknown} err
 * @returns {import('../index.js').GenerateError}
 */
function upgradeError(err) {
  if (GenerateError.isGenerateError(err)) return err;
  const message = field(err, 'message');
  const upgraded = new GenerateError({
    code: 'E_UNEXPECTED',
    message: message ? String(message) : String(err),
    warnings: [],
  });
  upgraded.cause = err;
  return upgraded;
}

/**
 * Fails with a typed `GenerateError` on an unknown key or a wrong shape.
 *
 * NAPI would reject a wrong type too, but with a generic "Failed to
 * convert"; failing here names the option and, where it helps, the value.
 *
 * @param {GenerateOptions} options
 * @returns {void}
 */
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

/**
 * Lowers `options.naming` without touching the caller's object. Returns
 * `undefined` when the key is absent, so it can stay absent.
 *
 * @param {GenerateOptions | null | undefined} options
 * @returns {import('../index.js').NamingOptions | undefined}
 */
function normalizeNaming(options) {
  if (options == null || typeof options !== 'object') return undefined;
  if (options.naming === undefined) return undefined;
  return {
    methodName: normalizeNamingValue(options.naming.methodName, 'methodName'),
    group: normalizeNamingValue(options.naming.group, 'group'),
  };
}

/**
 * Normalises and validates the options both entries pass to the binding:
 * applies the CLI-parity `emit` default, rewrites an `https` input into
 * `inputContents`, and runs the shape validator.
 *
 * @param {GenerateOptions} options
 * @param {(url: string) => Promise<import('./fetch-input.js').FetchedInput>} fetchInputFn
 *   Injected so the browser entry can refuse the fetch outright.
 * @returns {Promise<PreparedOptions>}
 */
async function prepareOptions(options, fetchInputFn) {
  // The default lands before validation, so the validator and the
  // boundary both see a populated emit set. A non-object passes through
  // for the binding's own type rejection.
  const normalized =
    options != null && typeof options === 'object' && options.emit === undefined
      ? { ...options, emit: [...DEFAULT_EMIT] }
      : options;

  // An `https` input is fetched and rewritten into `inputContents`
  // before validation, which treats that as the operative input. The
  // `http` case is refused inside `fetchInputFn`.
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
  unwrapOutcome,
  upgradeError,
  prepareOptions,
};
