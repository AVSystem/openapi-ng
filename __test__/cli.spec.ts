import test from 'ava';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath, pathToFileURL } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.join(__dirname, '..');
const fixture = (name: string) => path.join(repoRoot, 'test', 'fixtures', name);
const cliPath = path.join(repoRoot, 'bin', 'openapi-ng.js');

function runCli(
  args: string[],
  cwd = repoRoot,
  options: { env?: NodeJS.ProcessEnv } = {},
) {
  return spawnSync(process.execPath, [cliPath, ...args], {
    cwd,
    encoding: 'utf8',
    ...(options.env !== undefined ? { env: options.env } : {}),
  });
}

function withTempDir(run: (dir: string) => void) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'openapi-ng-cli-'));
  try {
    run(dir);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
}

// ── Success: stdout format ──────────────────────────────────────────────────

test('cli generate prints human-readable summary with title and file list', t => {
  withTempDir(outputPath => {
    const result = runCli([
      'generate',
      '--input',
      fixture('petstore-rich.openapi.yaml'),
      '--output',
      outputPath,
    ]);

    t.is(result.status, 0);
    t.is(result.stderr, '');
    t.true(result.stdout.includes('Generated 5 files from Petstore Rich'));
    t.true(result.stdout.includes('2 paths'));
    t.true(result.stdout.includes('3 operations'));
    t.true(result.stdout.includes('6 schemas'));
    t.true(result.stdout.includes('model.generated.ts'));
    t.true(result.stdout.includes('rest.model.ts'));
    t.true(result.stdout.includes('rest.util.ts'));
    t.true(result.stdout.includes('rest.validate.ts'));
    t.true(result.stdout.includes('rest/pet.rest.generated.ts'));
  });
});

test('cli generate prints summary for JSON fixture (JSON/YAML determinism)', t => {
  withTempDir(yamlOut => {
    withTempDir(jsonOut => {
      const yamlResult = runCli([
        'generate',
        '--input',
        fixture('petstore-rich.openapi.yaml'),
        '--output',
        yamlOut,
      ]);
      const jsonResult = runCli([
        'generate',
        '--input',
        fixture('petstore-rich.openapi.json'),
        '--output',
        jsonOut,
      ]);
      t.is(yamlResult.status, 0);
      t.is(jsonResult.status, 0);
      // Both summaries describe the same spec
      t.true(yamlResult.stdout.includes('Petstore Rich'));
      t.true(jsonResult.stdout.includes('Petstore Rich'));
    });
  });
});

test('cli generate prints summary without --output (in-memory, no file writing)', t => {
  withTempDir(tmpDir => {
    const result = runCli(
      ['generate', '--input', fixture('petstore-minimal.openapi.yaml')],
      tmpDir,
    );
    t.is(result.status, 0);
    t.is(result.stderr, '');
    t.true(result.stdout.includes('Generated'));
    t.true(result.stdout.includes('Petstore Minimal'));
    // No files should be written since --output was not provided
    t.deepEqual(fs.readdirSync(tmpDir), []);
  });
});

// ── Success: files on disk ──────────────────────────────────────────────────

test('cli generate writes model and service files to --output dir', t => {
  withTempDir(outputPath => {
    const result = runCli([
      'generate',
      '--input',
      fixture('petstore-rich.openapi.yaml'),
      '--output',
      outputPath,
    ]);
    t.is(result.status, 0);
    t.is(result.stderr, '');
    t.true(fs.existsSync(path.join(outputPath, 'model.generated.ts')));
    t.true(fs.existsSync(path.join(outputPath, 'rest.model.ts')));
    t.true(fs.existsSync(path.join(outputPath, 'rest.util.ts')));
    t.true(fs.existsSync(path.join(outputPath, 'rest', 'pet.rest.generated.ts')));
  });
});

test('cli generate --emit angular auto-includes models (with warning under --verbose)', t => {
  withTempDir(outputPath => {
    const result = runCli([
      'generate',
      '--input',
      fixture('petstore-minimal.openapi.yaml'),
      '--output',
      outputPath,
      '--emit',
      'angular',
      '--verbose',
    ]);
    t.is(result.status, 0);
    t.is(result.stderr, '');
    t.true(fs.existsSync(path.join(outputPath, 'model.generated.ts')));
    t.true(result.stdout.includes("Auto-included 'models'"));
    t.true(result.stdout.includes('E_INVALID_OPTION'));
  });
});

test('cli generate --mapped-type replaces schema with import', t => {
  withTempDir(outputPath => {
    const result = runCli([
      'generate',
      '--input',
      fixture('petstore-rich.openapi.yaml'),
      '--output',
      outputPath,
      '--mapped-type',
      'PetId:@demo/types:ExternalPetId',
    ]);
    t.is(result.status, 0);
    t.is(result.stderr, '');
    const modelContents = fs.readFileSync(
      path.join(outputPath, 'model.generated.ts'),
      'utf8',
    );
    t.true(modelContents.includes("import type { ExternalPetId } from '@demo/types'"));
    t.true(modelContents.includes('ExternalPetId'));
    t.false(modelContents.includes('export type PetId = string;'));
  });
});

test('cli generate writes 3 artifacts for fixture without operations', t => {
  withTempDir(outputPath => {
    const result = runCli([
      'generate',
      '--input',
      fixture('empty-shapes.openapi.yaml'),
      '--output',
      outputPath,
    ]);
    t.is(result.status, 0);
    t.true(fs.existsSync(path.join(outputPath, 'model.generated.ts')));
    t.true(fs.existsSync(path.join(outputPath, 'rest.model.ts')));
    t.true(fs.existsSync(path.join(outputPath, 'rest.util.ts')));
    t.false(fs.existsSync(path.join(outputPath, 'rest')));
  });
});

// ── Verbose: warnings ──────────────────────────────────────────────────────

test('cli generate suppresses warnings without --verbose', t => {
  // cookie-param emits a non-fatal warning (cookies aren't surfaced in the
  // generated service contract — browsers manage cookies via the cookie
  // store). header-param used to share this behaviour but headers are now
  // first-class.
  const result = runCli(['generate', '--input', fixture('cookie-param.openapi.yaml')]);
  t.is(result.status, 0);
  t.is(result.stderr, '');
  t.false(result.stdout.includes('Warnings'));
  t.false(result.stdout.includes('E_UNSUPPORTED_SEMANTIC'));
});

test('cli generate --verbose prints warnings with code and operationId', t => {
  const result = runCli([
    'generate',
    '--input',
    fixture('cookie-param.openapi.yaml'),
    '--verbose',
  ]);
  t.is(result.status, 0);
  t.is(result.stderr, '');
  t.true(result.stdout.includes('Warnings (1):'));
  t.true(result.stdout.includes('[E_UNSUPPORTED_SEMANTIC]'));
  t.true(result.stdout.includes("operationId 'listPets'"));
  t.true(result.stdout.includes("'sessionId'"));
  t.true(result.stdout.includes("'cookie'"));
});

// ── Errors: stderr format & exit code ──────────────────────────────────────

test('cli generate exits 1 with human-readable error for unsupported semantic', t => {
  const result = runCli([
    'generate',
    '--input',
    fixture('unsupported-semantic.openapi.yaml'),
  ]);
  t.is(result.status, 1);
  t.is(result.stdout, '');
  t.true(result.stderr.includes('E_UNSUPPORTED_SEMANTIC'));
  t.true(result.stderr.includes('unsupported-semantic.openapi.yaml'));
});

test('cli generate exits 1 with human-readable error for malformed YAML', t => {
  const result = runCli(['generate', '--input', fixture('malformed.yaml')]);
  t.is(result.status, 1);
  t.is(result.stdout, '');
  t.true(result.stderr.length > 0);
});

test('cli generate exits 1 with human-readable error for unsupported root shape', t => {
  const result = runCli(['generate', '--input', fixture('unsupported-root.yaml')]);
  t.is(result.status, 1);
  t.is(result.stdout, '');
  t.true(result.stderr.includes('E_INPUT_INVALID'));
  t.true(result.stderr.includes('decode'));
});

// ── Argument parsing errors ─────────────────────────────────────────────────

test('cli generate exits 1 with readable error when --input is missing', t => {
  const result = runCli(['generate']);
  t.is(result.status, 1);
  t.is(result.stdout, '');
  t.true(result.stderr.includes('E_INVALID_OPTION'));
  t.true(result.stderr.includes('--input'));
});

test('cli generate exits 1 with readable error for unknown argument', t => {
  const result = runCli([
    'generate',
    '--input',
    fixture('petstore-minimal.openapi.yaml'),
    '--bogus',
  ]);
  t.is(result.status, 1);
  t.is(result.stdout, '');
  t.true(result.stderr.includes('E_INVALID_OPTION'));
});

test('cli generate exits 1 with readable error for malformed --mapped-type', t => {
  const result = runCli([
    'generate',
    '--input',
    fixture('petstore-minimal.openapi.yaml'),
    '--mapped-type',
    'bad-format',
  ]);
  t.is(result.status, 1);
  t.is(result.stdout, '');
  t.true(result.stderr.includes('E_INVALID_OPTION'));
});

test('cli generate exits 1 with helpful error for --mapped-type with too many colons', t => {
  // Windows-style absolute import paths like C:\some\path collide with the
  // colon-delimited CLI surface. The error message must point users at the
  // YAML/JSON config file as the supported workaround (C1).
  const result = runCli([
    'generate',
    '--input',
    fixture('petstore-minimal.openapi.yaml'),
    '--mapped-type',
    'PetId:C:\\Users\\x\\types:ExternalPetId:Alias',
  ]);
  t.is(result.status, 1);
  t.is(result.stdout, '');
  t.true(result.stderr.includes('Invalid --mapped-type'));
  t.true(result.stderr.includes('mappedTypes:'));
  t.true(result.stderr.includes('config file'));
});

test('cli generate exits 1 with readable error for --mapped-type with empty segment', t => {
  const result = runCli([
    'generate',
    '--input',
    fixture('petstore-minimal.openapi.yaml'),
    '--mapped-type',
    'PetId::ExternalPetId',
  ]);
  t.is(result.status, 1);
  t.is(result.stdout, '');
  t.true(result.stderr.includes('Invalid --mapped-type'));
});

test('cli prints usage for --help', t => {
  const result = runCli(['--help']);
  t.is(result.status, 0);
  t.true(result.stdout.includes('Usage'));
  t.true(result.stdout.includes('--input'));
});

test('cli with no args prints help to stdout and exits 2', t => {
  // Bare `openapi-ng` should not silently succeed: CI scripts like
  // `openapi-ng generate ... && next-step` would otherwise run `next-step`
  // if the `generate` argv got eaten. Exit 2 is the conventional code for
  // a usage error (matches GNU getopt, argparse, etc.).
  const result = runCli([]);
  t.is(result.status, 2);
  t.regex(result.stdout, /Usage:/);
});

test('cli --help still exits 0', t => {
  // Pin the existing behaviour so the "no args" change does not bleed
  // into the explicit-help path.
  const result = runCli(['--help']);
  t.is(result.status, 0);
});

test('cli --version prints the package version and exits 0', t => {
  const result = runCli(['--version']);
  t.is(result.status, 0);
  t.regex(result.stdout, /^\d+\.\d+\.\d+/);
  t.is(result.stderr, '');
});

test('cli generate --help prints subcommand-specific usage with every flag', t => {
  const result = runCli(['generate', '--help']);
  t.is(result.status, 0);
  t.is(result.stderr, '');
  // Mentions the subcommand by name so users know they got per-subcommand help.
  t.true(result.stdout.toLowerCase().includes('generate'));
  // Every documented flag must appear in the per-subcommand help.
  for (const flag of [
    '--input',
    '--output',
    '--emit',
    '--mapped-type',
    '--verbose',
    '--config',
    '--help',
  ]) {
    t.true(
      result.stdout.includes(flag),
      `generate --help must list ${flag}; got:\n${result.stdout}`,
    );
  }
});

test('cli generate -h is accepted as an alias for --help', t => {
  const result = runCli(['generate', '-h']);
  t.is(result.status, 0);
  t.true(result.stdout.includes('--input'));
});

test('cli --help output includes an Examples section', t => {
  const result = runCli(['--help']);
  t.is(result.status, 0);
  t.regex(result.stdout, /Examples:/);
  t.regex(result.stdout, /openapi-ng init/);
  t.regex(result.stdout, /openapi-ng generate/);
});

test('cli generate --help output includes an Examples section', t => {
  const result = runCli(['generate', '--help']);
  t.is(result.status, 0);
  t.regex(result.stdout, /Examples:/);
  t.regex(result.stdout, /openapi-ng generate/);
});

test('cli init --help prints subcommand-specific usage', t => {
  const result = runCli(['init', '--help']);
  t.is(result.status, 0);
  t.true(result.stdout.toLowerCase().includes('init'));
});

test('cli prints usage for unknown command', t => {
  const result = runCli(['unknown-command']);
  t.is(result.status, 1);
  t.is(result.stdout, '');
  t.true(result.stderr.includes('E_INVALID_OPTION'));
});

// ── init command ──────────────────────────────────────────────────────────────

test('cli init creates default config file in cwd', t => {
  withTempDir(dir => {
    const result = runCli(['init'], dir);
    t.is(result.status, 0);
    t.is(result.stderr, '');
    const configPath = path.join(dir, '.openapi-ng.yaml');
    t.true(fs.existsSync(configPath));
    const contents = fs.readFileSync(configPath, 'utf8');
    t.true(contents.includes('input:'));
    t.true(contents.includes('output:'));
    t.true(contents.includes('emit:'));
  });
});

test('cli init does not overwrite existing config file', t => {
  withTempDir(dir => {
    const configPath = path.join(dir, '.openapi-ng.yaml');
    fs.writeFileSync(configPath, 'input: custom.yaml\n', 'utf8');
    const result = runCli(['init'], dir);
    t.not(result.status, 0);
    t.regex(result.stdout + result.stderr, /Cannot init: existing config file/i);
    const contents = fs.readFileSync(configPath, 'utf8');
    t.is(contents, 'input: custom.yaml\n');
  });
});

test('cli init --format yaml writes .openapi-ng.yaml (default behaviour preserved)', t => {
  withTempDir(dir => {
    const result = runCli(['init'], dir);
    t.is(result.status, 0);
    t.true(fs.existsSync(path.join(dir, '.openapi-ng.yaml')));
    const contents = fs.readFileSync(path.join(dir, '.openapi-ng.yaml'), 'utf8');
    t.regex(contents, /^# openapi-ng configuration/m);
    t.regex(contents, /^input:/m);
  });
});

test('cli init --format json writes .openapi-ng.json', t => {
  withTempDir(dir => {
    const result = runCli(['init', '--format', 'json'], dir);
    t.is(result.status, 0);
    const target = path.join(dir, '.openapi-ng.json');
    t.true(fs.existsSync(target));
    const parsed = JSON.parse(fs.readFileSync(target, 'utf8'));
    t.truthy(parsed.input);
    t.truthy(parsed.output);
  });
});

test('cli init --format ts writes openapi-ng.config.mts with defineConfig + RegExp example', t => {
  withTempDir(dir => {
    const result = runCli(['init', '--format', 'ts'], dir);
    t.is(result.status, 0);
    const target = path.join(dir, 'openapi-ng.config.mts');
    t.true(fs.existsSync(target));
    t.false(fs.existsSync(path.join(dir, 'openapi-ng.config.ts')));
    const contents = fs.readFileSync(target, 'utf8');
    t.regex(contents, /import \{ defineConfig \} from '@avsystem\/openapi-ng\/config'/);
    t.regex(contents, /export default defineConfig\(\{/);
    t.regex(contents, /parse: \/\^\[\^_\]\+_\(\?<rest>.+\)\$\//);
  });
});

test('cli init --format js writes openapi-ng.config.mjs with JSDoc @type', t => {
  withTempDir(dir => {
    const result = runCli(['init', '--format', 'js'], dir);
    t.is(result.status, 0);
    const target = path.join(dir, 'openapi-ng.config.mjs');
    t.false(fs.existsSync(path.join(dir, 'openapi-ng.config.js')));
    t.true(fs.existsSync(target));
    const contents = fs.readFileSync(target, 'utf8');
    t.regex(contents, /@type \{import\('@avsystem\/openapi-ng'\)\.Config\}/);
  });
});

test('cli init --format ts aborts when .openapi-ng.yaml exists (cross-format)', t => {
  withTempDir(dir => {
    fs.writeFileSync(path.join(dir, '.openapi-ng.yaml'), 'input: a\n', 'utf8');
    const result = runCli(['init', '--format', 'ts'], dir);
    t.not(result.status, 0);
    t.regex(result.stdout + result.stderr, /existing config file/i);
    t.regex(result.stdout + result.stderr, /\.openapi-ng\.yaml/);
    t.false(fs.existsSync(path.join(dir, 'openapi-ng.config.mts')));
  });
});

test('cli init --format yaml aborts when openapi-ng.config.ts exists (cross-format)', t => {
  withTempDir(dir => {
    fs.writeFileSync(
      path.join(dir, 'openapi-ng.config.ts'),
      'export default {}\n',
      'utf8',
    );
    const result = runCli(['init', '--format', 'yaml'], dir);
    t.not(result.status, 0);
    t.regex(result.stdout + result.stderr, /openapi-ng\.config\.ts/);
  });
});

test('cli init aborts on any of the eight discoverable config names', t => {
  // Spot-check three at once: json, cjs, and mts each must block init.
  const names = ['.openapi-ng.json', 'openapi-ng.config.cjs', 'openapi-ng.config.mts'];
  for (const existing of names) {
    withTempDir(dir => {
      fs.writeFileSync(path.join(dir, existing), 'placeholder\n', 'utf8');
      const result = runCli(['init', '--format', 'ts'], dir);
      t.not(result.status, 0, `init should have aborted with ${existing} present`);
      t.regex(result.stdout + result.stderr, new RegExp(existing.replace(/\./g, '\\.')));
    });
  }
});

test('cli init --format zsh errors with allow-list', t => {
  withTempDir(dir => {
    const result = runCli(['init', '--format', 'zsh'], dir);
    t.not(result.status, 0);
    t.regex(result.stderr, /--format/);
    t.regex(result.stderr, /yaml.*json.*ts.*js/);
  });
});

test('cli init --format js writes ESM template regardless of package.json#type', t => {
  for (const pkgJson of ['{"type":"module"}', '{"type":"commonjs"}', '{"name":"x"}']) {
    withTempDir(dir => {
      fs.writeFileSync(path.join(dir, 'package.json'), pkgJson, 'utf8');
      const result = runCli(['init', '--format', 'js'], dir);
      t.is(result.status, 0);
      t.true(fs.existsSync(path.join(dir, 'openapi-ng.config.mjs')));
      const contents = fs.readFileSync(path.join(dir, 'openapi-ng.config.mjs'), 'utf8');
      t.regex(contents, /^export default \{/m);
      t.notRegex(contents, /module\.exports/);
    });
  }
});

// ── config file ───────────────────────────────────────────────────────────────

test('cli generate reads input from config file', t => {
  withTempDir(dir => {
    fs.writeFileSync(
      path.join(dir, '.openapi-ng.yaml'),
      `input: ${fixture('petstore-minimal.openapi.yaml')}\n`,
      'utf8',
    );
    const result = runCli(['generate'], dir);
    t.is(result.status, 0);
    t.is(result.stderr, '');
    t.true(result.stdout.includes('Petstore Minimal'));
  });
});

test('cli generate --input overrides config file input', t => {
  withTempDir(dir => {
    fs.writeFileSync(
      path.join(dir, '.openapi-ng.yaml'),
      'input: nonexistent.yaml\n',
      'utf8',
    );
    const result = runCli(
      ['generate', '--input', fixture('petstore-minimal.openapi.yaml')],
      dir,
    );
    t.is(result.status, 0);
    t.is(result.stderr, '');
    t.true(result.stdout.includes('Petstore Minimal'));
  });
});

test('cli generate reads array-form emit from config file', t => {
  withTempDir(dir => {
    fs.writeFileSync(
      path.join(dir, '.openapi-ng.yaml'),
      [
        `input: ${fixture('petstore-rich.openapi.yaml')}`,
        `output: ${dir}`,
        'emit:',
        '  - models',
        '  - angular',
      ].join('\n') + '\n',
      'utf8',
    );
    const result = runCli(['generate'], dir);
    t.is(result.status, 0);
    t.is(result.stderr, '');
    t.true(result.stdout.includes('model.generated.ts'));
    t.true(fs.existsSync(path.join(dir, 'model.generated.ts')));
    t.true(fs.existsSync(path.join(dir, 'rest', 'pet.rest.generated.ts')));
  });
});

test('cli generate --config uses explicit config path', t => {
  withTempDir(dir => {
    const customConfig = path.join(dir, 'custom-config.yaml');
    fs.writeFileSync(
      customConfig,
      `input: ${fixture('petstore-minimal.openapi.yaml')}\n`,
      'utf8',
    );
    const result = runCli(['generate', '--config', customConfig], dir);
    t.is(result.status, 0);
    t.is(result.stderr, '');
    t.true(result.stdout.includes('Petstore Minimal'));
  });
});

test('cli generate reads mappedTypes from config file', t => {
  withTempDir(dir => {
    fs.writeFileSync(
      path.join(dir, '.openapi-ng.yaml'),
      [
        `input: ${fixture('petstore-rich.openapi.yaml')}`,
        `output: ${dir}`,
        'mappedTypes:',
        '  - schema: PetId',
        "    import: '@demo/types'",
        '    type: ExternalPetId',
      ].join('\n') + '\n',
      'utf8',
    );
    const result = runCli(['generate'], dir);
    t.is(result.status, 0);
    t.is(result.stderr, '');
    const modelContents = fs.readFileSync(path.join(dir, 'model.generated.ts'), 'utf8');
    t.true(modelContents.includes("import type { ExternalPetId } from '@demo/types'"));
    t.false(modelContents.includes('export type PetId = string;'));
  });
});

test('cli generate ignores absent config file', t => {
  withTempDir(dir => {
    const result = runCli(
      ['generate', '--input', fixture('petstore-minimal.openapi.yaml')],
      dir,
    );
    t.is(result.status, 0);
    t.is(result.stderr, '');
    t.true(result.stdout.includes('Petstore Minimal'));
  });
});

test('cli generate reads input from openapi-ng.config.ts (end-to-end)', t => {
  // Smoke test gated on native TS support; equivalent .mjs test below covers
  // older Node so the contract is verified somewhere on every CI matrix entry.
  const [nodeMajor, nodeMinor] = process.versions.node.split('.').map(Number);
  if (!(nodeMajor > 22 || (nodeMajor === 22 && nodeMinor >= 6))) {
    t.pass('skipped: Node native TS stripping requires 22.6+');
    return;
  }
  withTempDir(dir => {
    // Node's ESM loader requires a file:// URL or a forward-slash specifier;
    // a raw Windows path like 'D:\\…/lib/config.js' is interpreted as scheme
    // 'd:' and rejected with "Only URLs with a scheme in: file, data, and
    // node are supported".
    const configImport = pathToFileURL(path.join(repoRoot, 'lib', 'config.js')).href;
    fs.writeFileSync(
      path.join(dir, 'openapi-ng.config.ts'),
      `import { defineConfig } from '${configImport}';\n` +
        `export default defineConfig({\n` +
        `  input: ${JSON.stringify(fixture('petstore-rich.openapi.yaml'))},\n` +
        `  output: ${JSON.stringify(path.join(dir, 'out'))},\n` +
        `});\n`,
      'utf8',
    );
    const result = runCli(['generate'], dir);
    t.is(result.status, 0, result.stderr);
    t.true(fs.existsSync(path.join(dir, 'out', 'model.generated.ts')));
  });
});

test('cli generate reads input from openapi-ng.config.mjs (end-to-end)', t => {
  withTempDir(dir => {
    fs.writeFileSync(
      path.join(dir, 'openapi-ng.config.mjs'),
      `export default {\n` +
        `  input: ${JSON.stringify(fixture('petstore-rich.openapi.yaml'))},\n` +
        `  output: ${JSON.stringify(path.join(dir, 'out'))},\n` +
        `};\n`,
      'utf8',
    );
    const result = runCli(['generate'], dir);
    t.is(result.status, 0, result.stderr);
    t.true(fs.existsSync(path.join(dir, 'out', 'model.generated.ts')));
  });
});

test('cli generate honours naming.parse RegExp from openapi-ng.config.mjs', t => {
  // RegExp is the killer feature — we can express it now that JS/TS
  // configs exist. The fixture's operationIds don't have an underscore
  // prefix to strip, so the rule must fail gracefully and the fallback
  // (the second rule, which always matches) provides the method name.
  withTempDir(dir => {
    fs.writeFileSync(
      path.join(dir, 'openapi-ng.config.mjs'),
      `export default {\n` +
        `  input: ${JSON.stringify(fixture('petstore-rich.openapi.yaml'))},\n` +
        `  output: ${JSON.stringify(path.join(dir, 'out'))},\n` +
        `  naming: {\n` +
        `    methodName: [\n` +
        `      { from: '{operationId}', parse: /^[^_]+_(?<rest>.+)$/, format: '{capture.rest}', case: 'camel' },\n` +
        `      '{operationId}',\n` +
        `    ],\n` +
        `  },\n` +
        `};\n`,
      'utf8',
    );
    const result = runCli(['generate'], dir);
    t.is(result.status, 0, result.stderr);
  });
});

// ── Async-main rejection guard ────────────────────────────────────────────────

test('cli generate emits a clean error (not "unhandled rejection") when --config path is missing', t => {
  // Forces loadConfigFile to throw ENOENT. The inner try/catch in main
  // catches this today; the top-level .catch is the defense-in-depth
  // fallback for any future await in main that escapes a try/catch.
  // Contract under either path: exit 1, single readable stderr line, no
  // Node "[UnhandledPromiseRejection]" / multi-line stack dump.
  const result = runCli([
    'generate',
    '--config',
    '/definitely/nonexistent/openapi-ng.yaml',
  ]);
  t.is(result.status, 1);
  t.is(result.stdout, '');
  t.true(result.stderr.length > 0, 'expected a readable error on stderr');
  t.false(
    result.stderr.includes('UnhandledPromiseRejection'),
    `stderr leaked an unhandled-rejection header: ${result.stderr}`,
  );
  t.false(
    result.stderr.includes('node:internal'),
    `stderr leaked an internal Node stack frame: ${result.stderr}`,
  );
});

test('cli --config <missing-path> errors with E_INPUT_INVALID and a clean message', t => {
  const result = runCli(['generate', '--config', '/nonexistent/path-for-task-3-2.yaml']);
  t.is(result.status, 1);
  t.regex(result.stderr, /Error \[E_INPUT_INVALID\]/);
  t.regex(result.stderr, /config file not found/i);
  t.notRegex(result.stderr, /ENOENT/);
});

test('cli tags YAML parse errors in config as E_INPUT_INVALID', t => {
  withTempDir(dir => {
    fs.writeFileSync(path.join(dir, '.openapi-ng.yaml'), 'input: [unbalanced\n', 'utf8');
    const result = runCli(['generate'], dir);
    t.is(result.status, 1);
    t.regex(result.stderr, /Error \[E_INPUT_INVALID\]/);
    t.notRegex(result.stderr, /E_INVALID_OPTION/);
  });
});

test('cli surfaces config-file emit errors with the structured formatter', t => {
  withTempDir(dir => {
    fs.writeFileSync(
      path.join(dir, '.openapi-ng.yaml'),
      'input: x.yaml\nemit: [bogus]\n',
      'utf8',
    );
    const result = runCli(['generate'], dir);
    t.is(result.status, 1);
    t.regex(result.stderr, /Error \[E_INVALID_OPTION\]/);
    t.regex(result.stderr, /'bogus'/);
  });
});

// ── Lazy-load: native binding not loaded for non-generate commands ────────────

test('--help works without loading native binding', t => {
  const result = runCli(['--help'], repoRoot, {
    env: { ...process.env, OPENAPI_NG_DISABLE_NATIVE_FOR_TEST: '1' },
  });
  t.is(result.status, 0);
  t.true(result.stdout.includes('openapi-ng'));
  t.is(result.stderr, '');
});

test('--version works without loading native binding', t => {
  const result = runCli(['--version'], repoRoot, {
    env: { ...process.env, OPENAPI_NG_DISABLE_NATIVE_FOR_TEST: '1' },
  });
  t.is(result.status, 0);
  t.regex(result.stdout.trim(), /^\d+\.\d+\.\d+/);
});

test('cli --help mentions https URL input', t => {
  const result = runCli(['--help']);
  // Bare `openapi-ng` (no subcommand) is a usage error, exit 2 — but --help
  // is explicit, exit 0.
  t.is(result.status, 0);
  t.true(result.stdout.includes('https://'));
});

test('cli generate --help describes --input accepting path or url', t => {
  const result = runCli(['generate', '--help']);
  t.is(result.status, 0);
  t.true(result.stdout.includes('path|url'));
});
