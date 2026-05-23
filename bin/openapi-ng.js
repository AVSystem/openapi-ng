#!/usr/bin/env node

// Resolve through the wrapper so caught errors are `GenerateError`
// instances (the CLI formatter doesn't depend on `instanceof`, but
// consumers debugging via `node --inspect` see a consistent shape).
// NOTE: do NOT require('../lib/index.js') at module top — that would load
// the native binding on every invocation, including --help and --version.
// Use loadLibrary() inside the generate handler instead.
function loadLibrary() {
  return require('../lib/index.js');
}

const fs = require('node:fs');
const path = require('node:path');
const {
  CONFIG_FILENAMES,
  discoverConfigPath,
  loadConfigFile,
  mergeConfig,
  parseArgs,
} = require('./lib/parse.js');

// Minimal ANSI styler — emit colour only when stdout is a TTY and NO_COLOR is unset
const USE_COLOR = process.stdout.isTTY === true && !process.env.NO_COLOR;
const wrap = code => (USE_COLOR ? s => `\x1b[${code}m${s}\x1b[0m` : s => String(s));
const c = {
  bold: wrap(1),
  dim: wrap(2),
  red: wrap(31),
  green: wrap(32),
  yellow: wrap(33),
  cyan: wrap(36),
};

function printUsage() {
  process.stdout.write(
    [
      `${c.bold('Usage:')}`,
      `  openapi-ng generate [--input <path>] [--output <dir>] [--verbose]`,
      `                     [--emit <targets>] [--config <path>]`,
      `                     [--mapped-type <schemaName:importPath:typeName>]`,
      `  openapi-ng init [--format yaml|json|ts|js]`,
      '',
      `${c.bold('Global flags:')}`,
      `  --config, -c  Path to config file (default: auto-discover openapi-ng.config.{ts,mts,cts,mjs,js,cjs} or .openapi-ng.yaml/.json)`,
      `  --help, -h    Print this help message (works on any subcommand)`,
      `  --version, -v Print the openapi-ng package version and exit`,
      '',
      `${c.bold('Emit targets:')}`,
      `  --emit models,angular       Comma-separated list (repeatable). Default: 'models,angular'.`,
      `                              'angular' depends on 'models'; it is auto-included.`,
      '',
      `${c.bold('Supported inputs:')}`,
      '  - Local OpenAPI 3.x JSON or YAML files within the current subset.',
      '  - https:// URLs pointing to a JSON or YAML OpenAPI 3.x document.',
      '',
      `${c.bold('Examples:')}`,
      `  openapi-ng init [--format yaml|json|ts|js]   Create a starter config file`,
      `  openapi-ng generate -i spec.yaml -o ./src/generated`,
      `  openapi-ng generate --emit models --output ./tmp`,
    ].join('\n'),
  );
  process.stdout.write('\n');
}

function printGenerateUsage() {
  process.stdout.write(
    [
      `${c.bold('Usage:')}`,
      `  openapi-ng generate [flags]`,
      '',
      `${c.bold('Flags:')}`,
      `  --input, -i <path|url>      Path to an OpenAPI spec file, or an https:// URL.`,
      `  --output, -o <dir>          Output directory for generated files.`,
      `                              Omit to run in-memory (no files written).`,
      `  --emit <targets>            Comma-separated list (repeatable). Default: 'models,angular'.`,
      `                              Valid: 'models', 'angular'.`,
      `                              'angular' depends on 'models'; it is auto-included.`,
      `  --mapped-type <s:i:t[:a]>   Map schema <s> to imported type <t> from path <i>,`,
      `                              optionally renamed to <a>. Repeatable.`,
      `  --verbose                   Include warnings in the success summary.`,
      `  --config, -c <path>         Path to config file (overrides auto-discovery).`,
      `  --help, -h                  Print this help.`,
      `  --version, -v               Print the openapi-ng package version and exit.`,
      '',
      `${c.bold('Examples:')}`,
      `  openapi-ng generate -i spec.yaml -o ./src/generated`,
      `  openapi-ng generate --input spec.yaml --output ./tmp --verbose`,
      `  openapi-ng generate --config my-openapi-ng.yaml`,
    ].join('\n'),
  );
  process.stdout.write('\n');
}

function printInitUsage() {
  process.stdout.write(
    [
      `${c.bold('Usage:')}`,
      `  openapi-ng init [--format <kind>]`,
      '',
      `${c.bold('Flags:')}`,
      `  --format <kind>   yaml (default) | json | ts | js`,
      '',
      `Writes a starter config file in the current directory.`,
      `Aborts if a same-name config file already exists.`,
    ].join('\n'),
  );
  process.stdout.write('\n');
}

function formatSuccess(result, verbose) {
  const { summary, artifacts, diagnostics } = result;
  const count = artifacts.length;
  const lines = [
    `${c.bold(c.green('✓'))} Generated ${c.bold(count)} file${count !== 1 ? 's' : ''} from ${c.bold(summary.title)} ${c.dim(`(${summary.specVersion})`)}`,
    `  ${c.dim(`${summary.pathCount} path${summary.pathCount !== 1 ? 's' : ''} · ${summary.operationCount} operation${summary.operationCount !== 1 ? 's' : ''} · ${summary.schemaCount} schema${summary.schemaCount !== 1 ? 's' : ''}`)}`,
    '',
  ];
  for (const artifact of artifacts) {
    lines.push(`  ${c.cyan(artifact.path)}`);
  }
  if (verbose) {
    const warnings = diagnostics.filter(d => d.severity === 'warning');
    if (warnings.length > 0) {
      lines.push('', c.bold(c.yellow(`Warnings (${warnings.length}):`)));
      for (const w of warnings) {
        lines.push(`  ${c.yellow(`[${w.code}]`)} ${w.message}`);
      }
    }
  }
  return lines.join('\n');
}

// ── Init command ────────────────────────────────────────────────────────────

const DEFAULT_YAML_CONFIG = `# openapi-ng configuration
# https://github.com/AVSystem/openapi-ng

input: ./openapi.yaml
# input: https://example.com/openapi.yaml   # https:// URLs are also accepted
output: ./src/generated

# emit: list of artifact families to produce.
# Valid entries: 'models', 'angular'. 'angular' depends on 'models';
# it is auto-included if you omit it.
emit:
  - models
  - angular

# mappedTypes:
#   - schema: DateTime
#     import: dayjs
#     type: Dayjs

# naming:
#   methodName: '{operationId}'
#   group:
#     - format: '{tags[0]}'
#       case: pascal
#     - format: '{pathSegments[0]}'
#       case: pascal
`;

const DEFAULT_JSON_CONFIG =
  JSON.stringify(
    {
      input: './openapi.yaml',
      output: './src/generated',
      emit: ['models', 'angular'],
    },
    null,
    2,
  ) + '\n';

const DEFAULT_TS_CONFIG = `import { defineConfig } from '@avsystem/openapi-ng/config';

export default defineConfig({
  input: './openapi.yaml',
  // input: 'https://example.com/openapi.yaml',  // https:// URLs are also accepted
  output: './src/generated',

  // emit: ['models', 'angular'],

  // mappedTypes: [
  //   { schema: 'DateTime', import: 'dayjs', type: 'Dayjs' },
  // ],

  // naming: {
  //   methodName: {
  //     from: '{operationId}',
  //     parse: /^[^_]+_(?<rest>.+)$/,
  //     format: '{capture.rest}',
  //     case: 'camel',
  //   },
  //   group: [
  //     { format: '{tags[0]}', case: 'pascal' },
  //     { format: '{pathSegments[0]}', case: 'pascal' },
  //   ],
  // },
});
`;

const DEFAULT_JS_CONFIG = `/** @type {import('@avsystem/openapi-ng').Config} */
export default {
  input: './openapi.yaml',
  // input: 'https://example.com/openapi.yaml',  // https:// URLs are also accepted
  output: './src/generated',

  // emit: ['models', 'angular'],
};
`;

function runInit(format) {
  const cwd = process.cwd();
  const existing = CONFIG_FILENAMES.find(name => fs.existsSync(path.join(cwd, name)));
  if (existing !== undefined) {
    process.stderr.write(
      `${c.bold(c.red('Error'))} ${c.red('[E_INPUT_INVALID]')}\n` +
        `  Cannot init: existing config file found at ${c.bold(existing)}\n` +
        `  Remove it first, or use --config <path> with the existing file.\n`,
    );
    process.exitCode = 1;
    return;
  }

  let filename;
  let template;
  switch (format) {
    case 'yaml':
      filename = '.openapi-ng.yaml';
      template = DEFAULT_YAML_CONFIG;
      break;
    case 'json':
      filename = '.openapi-ng.json';
      template = DEFAULT_JSON_CONFIG;
      break;
    case 'ts':
      filename = 'openapi-ng.config.mts';
      template = DEFAULT_TS_CONFIG;
      break;
    case 'js':
      filename = 'openapi-ng.config.mjs';
      template = DEFAULT_JS_CONFIG;
      break;
    default:
      throw new Error(`Unknown init format: ${format}`);
  }

  fs.writeFileSync(path.join(cwd, filename), template, 'utf8');
  process.stdout.write(`${c.bold(c.green('✓'))} Created ${c.bold(filename)}\n`);
}

// ── Main ────────────────────────────────────────────────────────────────────

async function main(argv) {
  let parsed;

  try {
    parsed = parseArgs(argv);
  } catch (error) {
    process.stderr.write(`${formatParseFailure(error)}\n`);
    process.exitCode = 1;
    return;
  }

  if (parsed.kind === 'help') {
    if (parsed.subcommand === 'generate') {
      printGenerateUsage();
    } else if (parsed.subcommand === 'init') {
      printInitUsage();
    } else {
      printUsage();
    }
    // Bare `openapi-ng` (no subcommand) is a usage error — exit 2 so CI
    // scripts can catch a missing command. Explicit `--help` keeps exit 0.
    if (parsed.explicit === false) {
      process.exitCode = 2;
    }
    return;
  }

  if (parsed.kind === 'version') {
    const pkg = require('../package.json');
    process.stdout.write(`${pkg.version}\n`);
    return;
  }

  if (parsed.kind === 'init') {
    runInit(parsed.format);
    return;
  }

  // Load config file for generate command
  let fileConfig = {};
  try {
    const configFilePath = parsed.configPath ?? discoverConfigPath(process.cwd());
    if (configFilePath) {
      fileConfig = await loadConfigFile(configFilePath);
    }
  } catch (error) {
    process.stderr.write(`${formatParseFailure(error)}\n`);
    process.exitCode = 1;
    return;
  }

  // Generate command
  let merged;
  try {
    merged = mergeConfig(fileConfig, parsed);
    if (!merged.inputPath) {
      throw new Error('Missing required --input <path> argument.');
    }
  } catch (error) {
    process.stderr.write(`${formatParseFailure(error)}\n`);
    process.exitCode = 1;
    return;
  }

  try {
    const { generate } = loadLibrary();
    // Pass the user-provided inputPath verbatim. Relativisation of
    // absolute paths inside CWD (for the generated-artifact banner) is
    // owned by the Rust side in `render_generated_banner`, so the CLI
    // and programmatic consumers (`generate({ inputPath: '/abs/...' })`)
    // get the same banner-path hygiene without duplicated logic.
    const result = await generate({
      inputPath: merged.inputPath,
      outputPath: merged.outputPath ?? undefined,
      emit: merged.emit,
      mappedTypes: merged.mappedTypes ?? undefined,
      responseTypeMapping: merged.responseTypeMapping ?? undefined,
      naming: merged.naming ?? undefined,
    });
    process.stdout.write(`${formatSuccess(result, merged.verbose)}\n`);
  } catch (error) {
    process.stderr.write(`${formatFailure(error)}\n`);
    process.exitCode = 1;
  }
}

function formatParseFailure(error) {
  // Honour error.code when set (e.g. loadConfigFile tags ENOENT and
  // YAML/JSON parse failures with E_INPUT_INVALID — those are user
  // input problems, not CLI option-parsing problems). Fall back to
  // E_INVALID_OPTION only when no code is set, which is the
  // parseArgs-raised case for genuinely bad flags.
  const code = typeof error?.code === 'string' ? error.code : 'E_INVALID_OPTION';
  const message = typeof error?.message === 'string' ? error.message : String(error);
  return `${c.bold(c.red('Error'))} ${c.red(`[${code}]`)}\n  ${message}`;
}

function formatFailure(error) {
  if (typeof error?.code === 'string') {
    const lines = [
      `${c.bold(c.red('Error'))} ${c.red(`[${error.code}]`)}`,
      `  ${error.message}`,
    ];
    const errorPath = error.path ?? error.warnings?.[0]?.path;
    if (errorPath) {
      lines.push(`  ${c.dim(`in: ${errorPath}`)}`);
    }
    return lines.join('\n');
  }
  if (typeof error?.message === 'string') {
    return `${c.bold(c.red('Error'))} ${c.red('[E_UNEXPECTED]')}\n  ${error.message}`;
  }
  return `${c.bold(c.red('Error'))} ${c.red('[E_UNEXPECTED]')}\n  ${String(error)}`;
}

// Defense-in-depth: surface any error escaping `main` (e.g. a future
// `await` added without a local try/catch) as a single human-readable
// stderr line and exit 1 — never the Node default "[UnhandledPromise
// Rejection]" multi-line stack dump. Today the inner paths all catch
// their own failures; this is the last-line guard.
main(process.argv.slice(2)).catch(err => {
  process.stderr.write(`openapi-ng: ${err?.message ?? err}\n`);
  process.exitCode = 1;
});
