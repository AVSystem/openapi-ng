// CLI argument and config parsing helpers extracted from openapi-ng.js.
// Kept as a separate module so they are unit-testable without spawning
// a subprocess and without coupling to the runtime/output surface.
//
// The exported records mirror the `MappedType` shape on the NAPI
// boundary (`schema/import/type/alias`) one-to-one — the CLI does not
// translate between naming worlds.

const fs = require('node:fs');
const path = require('node:path');

const { field, inputError } = require('../../lib/diagnostic.js');

/** @typedef {import('../../index.js').Config} Config */
/** @typedef {import('../../index.js').EmitTarget} EmitTarget */
/** @typedef {import('../../index.js').MappedType} MappedType */

/**
 * What `parseArgs` resolved the command line to.
 *
 * @typedef {{ kind: 'version' }} ParsedVersion
 * @typedef {{ kind: 'help', subcommand: 'generate' | 'init' | null, explicit: boolean }} ParsedHelp
 * @typedef {{ kind: 'init', format: string }} ParsedInit
 * @typedef {{
 *   kind: 'generate',
 *   inputPath: string | null,
 *   outputPath: string | null,
 *   verbose: boolean | null,
 *   emit: EmitTarget[] | null,
 *   mappedTypes: MappedType[] | null,
 *   configPath: string | null,
 *   naming?: import('../../index.js').NamingConfig | null,
 * }} ParsedGenerate
 * @typedef {ParsedVersion | ParsedHelp | ParsedInit | ParsedGenerate} ParsedArgs
 */

/**
 * The generate request after the file config and the flags are merged.
 *
 * @typedef {{
 *   inputPath: string | null,
 *   outputPath: string | null,
 *   verbose: boolean,
 *   emit: EmitTarget[],
 *   mappedTypes: MappedType[] | null,
 *   responseTypeMapping: import('../../index.js').ResponseTypeMapping[] | null,
 *   naming: import('../../index.js').NamingConfig | null,
 * }} MergedConfig
 */

const VALID_EMIT_TARGETS = Object.freeze(new Set(['models', 'angular']));

/**
 * @param {string} value
 * @returns {value is EmitTarget}
 */
function isEmitTarget(value) {
  return VALID_EMIT_TARGETS.has(value);
}
/** @type {readonly EmitTarget[]} */
const DEFAULT_EMIT = Object.freeze(['models', 'angular']);

const VALID_INIT_FORMATS = Object.freeze(new Set(['yaml', 'json', 'ts', 'js']));

// Validate that argv[i + 1] is a real value, not the next flag or end-of-args.
// Without this, `--config --input spec.yaml` silently consumes `--input` as
// the config path, leaving the user staring at a config-not-found error
// without ever seeing their `--input` argument honoured. Treat any token
// starting with `-` (long `--foo` or short `-f`) as a flag, never a value.
/**
 * @param {readonly string[]} argv
 * @param {number} i Index of the flag itself.
 * @param {string} flagName Name printed in the failure.
 * @returns {string}
 */
function requireValue(argv, i, flagName) {
  const value = argv[i + 1];
  if (
    value === undefined ||
    value === null ||
    (typeof value === 'string' && value.startsWith('-'))
  ) {
    throw new Error(`${flagName} requires a value`);
  }
  return value;
}

// Normalize one user-supplied emit list (CLI comma-string or YAML
// array) into a deduped array of recognised targets. Unknown entries
// fail fast with a config-file hint.
/**
 * Normalises one emit list — a CLI comma-string or a config array — into
 * a deduped array of recognised targets. `null` when nothing was given.
 *
 * @param {unknown} value
 * @returns {EmitTarget[] | null}
 */
function normalizeEmit(value) {
  if (value === null || value === undefined) return null;

  let items;
  if (Array.isArray(value)) {
    items = value.map(v => String(v).trim()).filter(Boolean);
  } else if (typeof value === 'string') {
    items = value
      .split(',')
      .map(s => s.trim())
      .filter(Boolean);
  } else {
    throw new Error(
      `Invalid emit value: expected an array (YAML 'emit: [models, angular]') ` +
        `or comma-separated string ('--emit models,angular'); got ${typeof value}.`,
    );
  }

  /** @type {EmitTarget[]} */
  const targets = [];
  for (const item of items) {
    if (!isEmitTarget(item)) {
      throw new Error(`Unknown emit target: '${item}'. Allowed: 'models', 'angular'.`);
    }
    if (!targets.includes(item)) targets.push(item);
  }

  return targets;
}

/**
 * @param {string} value A `<schema:import:type(:alias)?>` triple or quad.
 * @returns {MappedType}
 */
function parseMappedType(value) {
  const source = String(value);
  // Reject up front: importPath segments may not contain ':' under the
  // colon-delimited CLI surface. Use mappedTypes: in the YAML/JSON config
  // file when import paths contain colons (e.g. Windows-style absolute
  // paths like C:\foo, or :: namespace separators).
  const parts = source.split(':');
  const [schema, importPath, typeName, alias] = parts;
  if (
    parts.length < 3 ||
    parts.length > 4 ||
    schema === undefined ||
    importPath === undefined ||
    typeName === undefined ||
    parts.some(part => part.length === 0)
  ) {
    throw new Error(
      `Invalid --mapped-type value: ${value}. Expected <schema:import:type(:alias)?>. ` +
        `For import paths containing ':' (e.g., Windows absolute paths), use the mappedTypes: ` +
        `entry in your .openapi-ng.yaml / .openapi-ng.json config file instead.`,
    );
  }

  return {
    schema,
    import: importPath,
    type: typeName,
    alias,
  };
}

// File names to probe at each level of the directory walk. Order matters:
// the first existing file wins. Modern config-style names (no dotfile
// prefix, matches vite/vitest/jest convention) rank above the legacy
// dotfile names so a project mid-migration prefers the richer JS/TS
// config when both are present.
const CONFIG_FILENAMES = Object.freeze([
  'openapi-ng.config.ts',
  'openapi-ng.config.mts',
  'openapi-ng.config.cts',
  'openapi-ng.config.mjs',
  'openapi-ng.config.js',
  'openapi-ng.config.cjs',
  '.openapi-ng.yaml',
  '.openapi-ng.json',
]);

/**
 * Walks up from `startDir` for the first recognised config file.
 *
 * @param {string} startDir
 * @returns {string | null}
 */
function discoverConfigPath(startDir) {
  let dir = path.resolve(startDir);
  let prev;

  while (prev !== dir) {
    for (const name of CONFIG_FILENAMES) {
      const candidate = path.join(dir, name);
      if (fs.existsSync(candidate)) return candidate;
    }
    prev = dir;
    dir = path.dirname(dir);
  }

  return null;
}

const JS_EXTENSIONS = new Set(['.js', '.mjs', '.cjs', '.ts', '.mts', '.cts']);

/**
 * Loads a config file, dispatching on its extension.
 *
 * @param {string} configPath
 * @returns {Promise<Config>}
 */
async function loadConfigFile(configPath) {
  const ext = path.extname(configPath).toLowerCase();

  // ── JS/TS branch: dynamic import + default-export validation ───────────
  if (JS_EXTENSIONS.has(ext)) {
    const { pathToFileURL } = require('node:url');
    const absPath = path.resolve(configPath);

    // Existence check up front. Dynamic import() raises an ERR_MODULE_NOT_FOUND
    // that includes the resolved URL — wrap it in our shape so the CLI
    // formatter prints the same "Config file not found:" line YAML/JSON
    // produces today.
    if (!fs.existsSync(absPath)) {
      throw inputError(`Config file not found: ${configPath}`);
    }

    let mod;
    try {
      mod = await import(pathToFileURL(absPath).href);
    } catch (err) {
      // TypeScript file types (.ts/.mts/.cts) on Node < 22.6 produce
      // ERR_UNKNOWN_FILE_EXTENSION because the loader has no idea what
      // to do with TypeScript. Map it to a version-aware hint instead
      // of the generic "Failed to load" wrap so users know the fix
      // (upgrade Node, switch to .js, or pass --experimental-strip-types).
      const isTs = ext === '.ts' || ext === '.mts' || ext === '.cts';
      if (isTs && field(err, 'code') === 'ERR_UNKNOWN_FILE_EXTENSION') {
        throw inputError(
          `TypeScript config files require Node 22.6+ with --experimental-strip-types, ` +
            `or Node 23.6+ (flag enabled by default). ` +
            `Alternatively, use a .js/.mjs config.`,
        );
      }
      throw inputError(`Failed to load config file ${configPath}: ${field(err, 'message') ?? err}`);
    }

    if (!('default' in mod) || mod.default === undefined) {
      throw inputError(
        `Config file ${configPath} has no default export. ` +
          `Use \`export default { ... }\` or \`module.exports = { ... }\`.`,
      );
    }

    let value = mod.default;
    // Allow `export default async () => ({...})` and `export default () => ({...})`.
    if (typeof value === 'function') {
      value = await value();
    }

    if (value === null || typeof value !== 'object' || Array.isArray(value)) {
      throw inputError(
        `Config file ${configPath} default export must be an object or function returning one; ` +
          `got ${value === null ? 'null' : Array.isArray(value) ? 'array' : typeof value}.`,
      );
    }

    return value;
  }

  // ── YAML/JSON branch (unchanged from before, factored out) ────────────
  // Both ENOENT (missing file) and YAML/JSON syntax errors are
  // user-input problems, not CLI option-parsing problems. Tag them
  // with E_INPUT_INVALID so formatParseFailure renders a stable code
  // for downstream tooling, and strip the raw POSIX message so we
  // don't leak filesystem internals to the terminal.
  let contents;
  try {
    contents = fs.readFileSync(configPath, 'utf8');
  } catch (err) {
    if (field(err, 'code') === 'ENOENT') {
      throw inputError(`Config file not found: ${configPath}`);
    }
    throw inputError(`Failed to read config file ${configPath}: ${field(err, 'message') ?? err}`);
  }

  try {
    if (ext === '.json') {
      return JSON.parse(contents);
    }

    const YAML = require('yaml');
    return YAML.parse(contents) ?? {};
  } catch (err) {
    throw inputError(`Failed to parse config file ${configPath}: ${field(err, 'message') ?? err}`);
  }
}

/**
 * @param {unknown} items
 * @returns {MappedType[] | null}
 */
function normalizeMappedTypes(items) {
  if (!Array.isArray(items)) return null;
  return items.map(item => ({
    schema: item.schema,
    import: item.import,
    type: item.type,
    alias: item.alias,
  }));
}

/**
 * @param {unknown} items
 * @returns {import('../../index.js').ResponseTypeMapping[] | null}
 */
function normalizeResponseTypeMapping(items) {
  if (!Array.isArray(items)) return null;
  return items.map(item => ({
    contentType: item.contentType,
    responseType: item.responseType,
  }));
}

/**
 * Accepts a naming block from a config file, refusing a `parse` that is
 * not a real `RegExp` — which is every value YAML or JSON can carry.
 *
 * @param {unknown} naming
 * @returns {import('../../index.js').NamingConfig | null}
 */
function normalizeNamingFromFile(naming) {
  if (naming === undefined || naming === null) return null;
  if (typeof naming !== 'object' || Array.isArray(naming)) {
    throw inputError(`Invalid naming config: expected an object with optional 'methodName' and 'group' keys.`);
  }
  // `parse` must be a JavaScript RegExp. JS/TS configs deliver one
  // directly; YAML/JSON cannot encode RegExp, so any `parse:` value
  // from those formats is a string/object/etc. and fails this check.
  // This keeps the "no parse in YAML/JSON" safety property without
  // tracking the source format through the call chain.
  for (const key of ['methodName', 'group']) {
    const value = field(naming, key);
    if (value === undefined) continue;
    const items = Array.isArray(value) ? value : [value];
    for (const item of items) {
      if (
        item &&
        typeof item === 'object' &&
        item.parse !== undefined &&
        !(item.parse instanceof RegExp)
      ) {
        throw inputError(
          `naming.${key}: 'parse' must be a JavaScript RegExp. ` +
            `YAML/JSON configs cannot encode RegExp — use an openapi-ng.config.ts ` +
            `(or .js/.mjs) file when you need 'parse' rules.`,
        );
      }
    }
  }
  return naming;
}

/**
 * Merges the file config under the CLI flags, which win field by field.
 *
 * @param {Config} fileConfig
 * @param {ParsedGenerate} cliFlags
 * @returns {MergedConfig}
 */
function mergeConfig(fileConfig, cliFlags) {
  /** @type {MergedConfig} */
  const merged = {
    inputPath: null,
    outputPath: null,
    verbose: false,
    emit: [...DEFAULT_EMIT],
    mappedTypes: null,
    responseTypeMapping: null,
    naming: null,
  };

  merged.inputPath = cliFlags.inputPath ?? fileConfig.input ?? null;
  merged.outputPath = cliFlags.outputPath ?? fileConfig.output ?? null;
  merged.verbose = cliFlags.verbose ?? false;

  const cliEmit = normalizeEmit(cliFlags.emit);
  const fileEmit = normalizeEmit(fileConfig.emit);
  merged.emit = cliEmit ?? fileEmit ?? [...DEFAULT_EMIT];

  const fileMappedTypes = normalizeMappedTypes(fileConfig.mappedTypes);
  merged.mappedTypes = cliFlags.mappedTypes ?? fileMappedTypes ?? null;

  merged.responseTypeMapping = normalizeResponseTypeMapping(
    fileConfig.responseTypeMapping,
  );

  merged.naming = cliFlags.naming ?? normalizeNamingFromFile(fileConfig.naming);

  return merged;
}

/**
 * @param {readonly string[]} argv Arguments after the executable name.
 * @returns {ParsedArgs}
 */
function parseArgs(argv) {
  let configPath = null;

  // `--version` / `-v` is a global flag: recognised anywhere in argv so
  // users can type `openapi-ng --version`, `openapi-ng generate --version`,
  // or `openapi-ng -v` and always get the version. Short-circuits before
  // any other parsing so it cannot trip on a half-finished command line.
  if (argv.some(token => token === '--version' || token === '-v')) {
    return { kind: 'version' };
  }

  // Extract global --config/-c before command parsing
  /** @type {string[]} */
  const filteredArgv = [];
  for (let i = 0; i < argv.length; i++) {
    const token = argv[i] ?? '';
    if (token === '--config' || token === '-c') {
      configPath = requireValue(argv, i, '--config');
      i += 1;
      continue;
    }
    filteredArgv.push(token);
  }

  const [command, ...rest] = filteredArgv;

  // Distinguish bare `openapi-ng` (no command + no global help flag) from
  // explicit `--help`/`-h`. CI scripts like `openapi-ng generate ... &&
  // next-step` would silently run `next-step` if the `generate` argv got
  // eaten; bare invocation is a usage error and must exit non-zero. We
  // still print help so the user can recover — only the exit code differs.
  // `explicit: false` is the signal for the caller to set `process.exitCode = 2`.
  if (!command) {
    return { kind: 'help', subcommand: null, explicit: false };
  }
  if (command === '--help' || command === '-h') {
    return { kind: 'help', subcommand: null, explicit: true };
  }

  if (command === 'init') {
    let format = 'yaml';
    for (let i = 0; i < rest.length; i += 1) {
      const token = rest[i] ?? '';
      if (token === '--help' || token === '-h') {
        return { kind: 'help', subcommand: 'init', explicit: true };
      }
      if (token === '--format') {
        const value = requireValue(rest, i, '--format');
        if (!VALID_INIT_FORMATS.has(value)) {
          throw new Error(
            `Unknown --format value: '${value}'. Allowed: 'yaml', 'json', 'ts', 'js'.`,
          );
        }
        format = value;
        i += 1;
        continue;
      }
      throw new Error(`Unsupported argument: ${token}`);
    }
    return { kind: 'init', format };
  }

  if (command !== 'generate') {
    throw new Error(`Unsupported command: ${command}`);
  }

  let inputPath = null;
  let outputPath = null;
  let verbose = null;
  const emitTokens = [];
  const mappedTypes = [];

  for (let index = 0; index < rest.length; index += 1) {
    const token = rest[index] ?? '';
    // Per-subcommand help short-circuit. Recognised anywhere in the
    // argument list so users can append `--help` to a half-finished
    // command without erasing the rest first.
    if (token === '--help' || token === '-h') {
      return { kind: 'help', subcommand: 'generate', explicit: true };
    }

    if (token === '--input' || token === '-i') {
      inputPath = requireValue(rest, index, '--input');
      index += 1;
      continue;
    }

    if (token === '--output' || token === '-o') {
      outputPath = requireValue(rest, index, '--output');
      index += 1;
      continue;
    }

    if (token === '--verbose') {
      verbose = true;
      continue;
    }

    if (token === '--emit') {
      const value = requireValue(rest, index, '--emit');
      emitTokens.push(value);
      index += 1;
      continue;
    }

    if (token === '--mapped-type') {
      mappedTypes.push(parseMappedType(requireValue(rest, index, '--mapped-type')));
      index += 1;
      continue;
    }

    throw new Error(`Unsupported argument: ${token}`);
  }

  // Normalise eagerly so unknown emit targets fail at parse time rather
  // than at validate time inside the Rust binding.
  const emit = emitTokens.length > 0 ? normalizeEmit(emitTokens.join(',')) : null;

  return {
    kind: 'generate',
    inputPath,
    outputPath,
    verbose,
    emit,
    mappedTypes: mappedTypes.length > 0 ? mappedTypes : null,
    configPath,
  };
}

module.exports = {
  parseMappedType,
  discoverConfigPath,
  loadConfigFile,
  normalizeMappedTypes,
  normalizeResponseTypeMapping,
  normalizeNamingFromFile,
  normalizeEmit,
  mergeConfig,
  parseArgs,
  DEFAULT_EMIT,
  CONFIG_FILENAMES,
};
