// CLI argument and config parsing helpers extracted from openapi-ng.js.
// Kept as a separate module so they are unit-testable without spawning
// a subprocess and without coupling to the runtime/output surface.
//
// The exported records mirror the `MappedType` shape on the NAPI
// boundary (`schema/import/type/alias`) one-to-one — the CLI does not
// translate between naming worlds.

const fs = require('node:fs');
const path = require('node:path');

const VALID_EMIT_TARGETS = Object.freeze(new Set(['models', 'angular']));
const DEFAULT_EMIT = Object.freeze(['models', 'angular']);

const VALID_INIT_FORMATS = Object.freeze(new Set(['yaml', 'json', 'ts', 'js']));

// Validate that argv[i + 1] is a real value, not the next flag or end-of-args.
// Without this, `--config --input spec.yaml` silently consumes `--input` as
// the config path, leaving the user staring at a config-not-found error
// without ever seeing their `--input` argument honoured. Treat any token
// starting with `-` (long `--foo` or short `-f`) as a flag, never a value.
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

  for (const item of items) {
    if (!VALID_EMIT_TARGETS.has(item)) {
      throw new Error(`Unknown emit target: '${item}'. Allowed: 'models', 'angular'.`);
    }
  }

  return Array.from(new Set(items));
}

function parseMappedType(value) {
  const source = String(value);
  // Reject up front: importPath segments may not contain ':' under the
  // colon-delimited CLI surface. Use mappedTypes: in the YAML/JSON config
  // file when import paths contain colons (e.g. Windows-style absolute
  // paths like C:\foo, or :: namespace separators).
  const parts = source.split(':');
  if (parts.length < 3 || parts.length > 4 || parts.some(part => part.length === 0)) {
    throw new Error(
      `Invalid --mapped-type value: ${value}. Expected <schema:import:type(:alias)?>. ` +
        `For import paths containing ':' (e.g., Windows absolute paths), use the mappedTypes: ` +
        `entry in your .openapi-ng.yaml / .openapi-ng.json config file instead.`,
    );
  }

  const [schema, importPath, typeName, alias] = parts;

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
      const e = new Error(`Config file not found: ${configPath}`);
      e.code = 'E_INPUT_INVALID';
      throw e;
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
      if (isTs && err?.code === 'ERR_UNKNOWN_FILE_EXTENSION') {
        const e = new Error(
          `TypeScript config files require Node 22.6+ with --experimental-strip-types, ` +
            `or Node 23.6+ (flag enabled by default). ` +
            `Alternatively, use a .js/.mjs config.`,
        );
        e.code = 'E_INPUT_INVALID';
        throw e;
      }
      const e = new Error(
        `Failed to load config file ${configPath}: ${err?.message ?? err}`,
      );
      e.code = 'E_INPUT_INVALID';
      throw e;
    }

    if (!('default' in mod) || mod.default === undefined) {
      const e = new Error(
        `Config file ${configPath} has no default export. ` +
          `Use \`export default { ... }\` or \`module.exports = { ... }\`.`,
      );
      e.code = 'E_INPUT_INVALID';
      throw e;
    }

    let value = mod.default;
    // Allow `export default async () => ({...})` and `export default () => ({...})`.
    if (typeof value === 'function') {
      value = await value();
    }

    if (value === null || typeof value !== 'object' || Array.isArray(value)) {
      const e = new Error(
        `Config file ${configPath} default export must be an object or function returning one; ` +
          `got ${value === null ? 'null' : Array.isArray(value) ? 'array' : typeof value}.`,
      );
      e.code = 'E_INPUT_INVALID';
      throw e;
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
    if (err && err.code === 'ENOENT') {
      const e = new Error(`Config file not found: ${configPath}`);
      e.code = 'E_INPUT_INVALID';
      throw e;
    }
    const e = new Error(
      `Failed to read config file ${configPath}: ${err?.message ?? err}`,
    );
    e.code = 'E_INPUT_INVALID';
    throw e;
  }

  try {
    if (ext === '.json') {
      return JSON.parse(contents);
    }

    const YAML = require('yaml');
    return YAML.parse(contents) ?? {};
  } catch (err) {
    const e = new Error(
      `Failed to parse config file ${configPath}: ${err?.message ?? err}`,
    );
    e.code = 'E_INPUT_INVALID';
    throw e;
  }
}

function normalizeMappedTypes(items) {
  if (!Array.isArray(items)) return null;
  return items.map(item => ({
    schema: item.schema,
    import: item.import,
    type: item.type,
    alias: item.alias,
  }));
}

function normalizeResponseTypeMapping(items) {
  if (!Array.isArray(items)) return null;
  return items.map(item => ({
    contentType: item.contentType,
    responseType: item.responseType,
  }));
}

function normalizeNamingFromFile(naming) {
  if (naming === undefined || naming === null) return null;
  if (typeof naming !== 'object' || Array.isArray(naming)) {
    const e = new Error(
      `Invalid naming config: expected an object with optional 'methodName' and 'group' keys.`,
    );
    e.code = 'E_INPUT_INVALID';
    throw e;
  }
  // `parse` must be a JavaScript RegExp. JS/TS configs deliver one
  // directly; YAML/JSON cannot encode RegExp, so any `parse:` value
  // from those formats is a string/object/etc. and fails this check.
  // This keeps the "no parse in YAML/JSON" safety property without
  // tracking the source format through the call chain.
  for (const key of ['methodName', 'group']) {
    const value = naming[key];
    if (value === undefined) continue;
    const items = Array.isArray(value) ? value : [value];
    for (const item of items) {
      if (
        item &&
        typeof item === 'object' &&
        item.parse !== undefined &&
        !(item.parse instanceof RegExp)
      ) {
        const e = new Error(
          `naming.${key}: 'parse' must be a JavaScript RegExp. ` +
            `YAML/JSON configs cannot encode RegExp — use an openapi-ng.config.ts ` +
            `(or .js/.mjs) file when you need 'parse' rules.`,
        );
        e.code = 'E_INPUT_INVALID';
        throw e;
      }
    }
  }
  return naming;
}

function mergeConfig(fileConfig, cliFlags) {
  const merged = {};

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
  const filteredArgv = [];
  for (let i = 0; i < argv.length; i++) {
    const token = argv[i];
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
      const token = rest[i];
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
    const token = rest[index];
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
