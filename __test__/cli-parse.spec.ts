import test from 'ava';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.join(__dirname, '..');
const require = createRequire(import.meta.url);

type ParseModule = {
  parseMappedType(value: string): {
    schema: string;
    import: string;
    type: string;
    alias?: string;
  };
  discoverConfigPath(startDir: string): string | null;
  loadConfigFile(configPath: string): Promise<Record<string, unknown>>;
  normalizeMappedTypes(items: unknown): unknown[] | null;
  normalizeEmit(value: unknown): string[] | null;
  mergeConfig(
    fileConfig: Record<string, unknown>,
    cliFlags: Record<string, unknown>,
  ): Record<string, unknown>;
  parseArgs(argv: string[]): Record<string, unknown>;
  DEFAULT_EMIT: readonly string[];
  CONFIG_FILENAMES: readonly string[];
};

const parse = require(path.join(repoRoot, 'bin', 'lib', 'parse.js')) as ParseModule;

async function withTempDir(run: (dir: string) => void | Promise<void>): Promise<void> {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'openapi-ng-parse-'));
  try {
    await run(dir);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
}

// ── parseMappedType ──────────────────────────────────────────────────────────

test('parseMappedType accepts 3-part schema:import:type', t => {
  t.deepEqual(parse.parseMappedType('PetId:@demo/types:ExternalPetId'), {
    schema: 'PetId',
    import: '@demo/types',
    type: 'ExternalPetId',
    alias: undefined,
  });
});

test('parseMappedType accepts 4-part schema:import:type:alias', t => {
  t.deepEqual(parse.parseMappedType('PetId:@demo/types:ExternalPetId:Alias'), {
    schema: 'PetId',
    import: '@demo/types',
    type: 'ExternalPetId',
    alias: 'Alias',
  });
});

test('parseMappedType rejects fewer than 3 parts with config-file hint', t => {
  const error = t.throws(() => parse.parseMappedType('PetId:OnlyOne'));
  t.true(error?.message.includes('Invalid --mapped-type'));
  t.true(error?.message.includes('mappedTypes:'));
  t.true(error?.message.includes('config file'));
});

test('parseMappedType rejects more than 4 parts (Windows-style absolute paths)', t => {
  const error = t.throws(() =>
    parse.parseMappedType('PetId:C:\\Users\\x\\types:ExternalPetId:Alias'),
  );
  t.true(error?.message.includes('Invalid --mapped-type'));
  t.true(error?.message.includes('Windows absolute paths'));
});

test('parseMappedType rejects empty segments', t => {
  const error = t.throws(() => parse.parseMappedType('PetId::ExternalPetId'));
  t.true(error?.message.includes('Invalid --mapped-type'));
});

// ── normalizeMappedTypes ─────────────────────────────────────────────────────

test('normalizeMappedTypes returns null for non-arrays', t => {
  t.is(parse.normalizeMappedTypes(undefined), null);
  t.is(parse.normalizeMappedTypes(null), null);
  t.is(parse.normalizeMappedTypes('PetId:@demo:Type'), null);
  t.is(parse.normalizeMappedTypes({}), null);
});

test('normalizeMappedTypes passes the schema/import/type/alias shape through unchanged', t => {
  t.deepEqual(
    parse.normalizeMappedTypes([
      { schema: 'PetId', import: '@demo', type: 'ExternalPetId', alias: 'Alias' },
    ]),
    [
      {
        schema: 'PetId',
        import: '@demo',
        type: 'ExternalPetId',
        alias: 'Alias',
      },
    ],
  );
});

test('normalizeMappedTypes leaves missing alias as undefined (not pulled from typeAlias)', t => {
  t.deepEqual(
    parse.normalizeMappedTypes([
      { schema: 'PetId', import: '@demo', type: 'ExternalPetId' },
    ]),
    [
      {
        schema: 'PetId',
        import: '@demo',
        type: 'ExternalPetId',
        alias: undefined,
      },
    ],
  );
});

// ── normalizeEmit ────────────────────────────────────────────────────────────

test('normalizeEmit returns null for null/undefined', t => {
  t.is(parse.normalizeEmit(null), null);
  t.is(parse.normalizeEmit(undefined), null);
});

test('normalizeEmit accepts YAML array form', t => {
  t.deepEqual(parse.normalizeEmit(['models', 'angular']), ['models', 'angular']);
  t.deepEqual(parse.normalizeEmit(['angular']), ['angular']);
});

test('normalizeEmit accepts CLI comma-separated string', t => {
  t.deepEqual(parse.normalizeEmit('models,angular'), ['models', 'angular']);
});

test('normalizeEmit trims whitespace and drops empties', t => {
  t.deepEqual(parse.normalizeEmit(' models , angular , '), ['models', 'angular']);
});

test('normalizeEmit dedupes while preserving order', t => {
  t.deepEqual(parse.normalizeEmit('models,angular,models'), ['models', 'angular']);
});

test('normalizeEmit rejects unknown targets', t => {
  const err = t.throws(() => parse.normalizeEmit(['models', 'react-query']));
  t.true(err?.message.includes("Unknown emit target: 'react-query'"));
  t.true(err?.message.includes("'models'"));
  t.true(err?.message.includes("'angular'"));
});

test('normalizeEmit rejects non-array, non-string values', t => {
  const err = t.throws(() => parse.normalizeEmit({ angular: true }));
  t.true(err?.message.includes('Invalid emit value'));
});

// ── mergeConfig ──────────────────────────────────────────────────────────────

test('mergeConfig: cli flags override file config', t => {
  const merged = parse.mergeConfig(
    { input: 'file-input.yaml', output: 'file-out', emit: ['models', 'angular'] },
    { inputPath: 'cli-input.yaml', outputPath: 'cli-out', emit: ['models'] },
  );
  t.is(merged.inputPath, 'cli-input.yaml');
  t.is(merged.outputPath, 'cli-out');
  t.deepEqual(merged.emit, ['models']);
});

test('mergeConfig: file config fills gaps when cli flag absent', t => {
  const merged = parse.mergeConfig(
    { input: 'file-input.yaml', emit: ['models', 'angular'] },
    { inputPath: null, outputPath: null, emit: null },
  );
  t.is(merged.inputPath, 'file-input.yaml');
  t.deepEqual(merged.emit, ['models', 'angular']);
});

test('mergeConfig: defaults emit to models+angular when both file and cli absent', t => {
  const merged = parse.mergeConfig({}, {});
  t.is(merged.inputPath, null);
  t.is(merged.outputPath, null);
  t.is(merged.verbose, false);
  t.deepEqual(merged.emit, ['models', 'angular']);
  t.is(merged.mappedTypes, null);
});

test('mergeConfig: cli emit wins over file emit', t => {
  const merged = parse.mergeConfig({ emit: ['models'] }, { emit: ['angular'] });
  t.deepEqual(merged.emit, ['angular']);
});

test('mergeConfig: cli mappedTypes wins over file mappedTypes', t => {
  const merged = parse.mergeConfig(
    {
      mappedTypes: [{ schema: 'A', import: 'a', type: 'A' }],
    },
    {
      mappedTypes: [{ schema: 'B', import: 'b', type: 'B', alias: undefined }],
    },
  );
  t.deepEqual(merged.mappedTypes, [
    { schema: 'B', import: 'b', type: 'B', alias: undefined },
  ]);
});

test('mergeConfig: file mappedTypes flow through when no cli override', t => {
  const merged = parse.mergeConfig(
    { mappedTypes: [{ schema: 'PetId', import: '@demo', type: 'ExternalPetId' }] },
    { mappedTypes: null },
  );
  t.deepEqual(merged.mappedTypes, [
    {
      schema: 'PetId',
      import: '@demo',
      type: 'ExternalPetId',
      alias: undefined,
    },
  ]);
});

test('mergeConfig: file responseTypeMapping flows through to merged config', t => {
  const merged = parse.mergeConfig(
    {
      responseTypeMapping: [
        { contentType: 'application/octet-stream', responseType: 'blob' },
        { contentType: 'text/csv', responseType: 'text' },
      ],
    },
    {},
  );
  t.deepEqual(merged.responseTypeMapping, [
    { contentType: 'application/octet-stream', responseType: 'blob' },
    { contentType: 'text/csv', responseType: 'text' },
  ]);
});

test('mergeConfig: responseTypeMapping is null when file omits it', t => {
  const merged = parse.mergeConfig({}, {});
  t.is(merged.responseTypeMapping, null);
});

// ── parseArgs ────────────────────────────────────────────────────────────────

test('parseArgs returns kind=help for empty argv, --help, -h', t => {
  t.is((parse.parseArgs([]) as { kind: string }).kind, 'help');
  t.is((parse.parseArgs(['--help']) as { kind: string }).kind, 'help');
  t.is((parse.parseArgs(['-h']) as { kind: string }).kind, 'help');
});

test('parseArgs flags empty argv as non-explicit help (drives exit 2)', t => {
  // Bare `openapi-ng` and explicit `--help` both classify as help, but the
  // caller needs to distinguish them so CI scripts can catch a missing
  // subcommand. The `explicit` field is that signal.
  t.is((parse.parseArgs([]) as { explicit: boolean }).explicit, false);
  t.is((parse.parseArgs(['--help']) as { explicit: boolean }).explicit, true);
  t.is((parse.parseArgs(['-h']) as { explicit: boolean }).explicit, true);
  t.is((parse.parseArgs(['generate', '--help']) as { explicit: boolean }).explicit, true);
  t.is((parse.parseArgs(['init', '--help']) as { explicit: boolean }).explicit, true);
});

test('parseArgs returns kind=init for `init`', t => {
  t.deepEqual(parse.parseArgs(['init']), { kind: 'init', format: 'yaml' });
});

test('parseArgs init defaults format to yaml when --format is absent', t => {
  t.like(parse.parseArgs(['init']), { kind: 'init', format: 'yaml' });
});

test('parseArgs init accepts --format yaml', t => {
  t.like(parse.parseArgs(['init', '--format', 'yaml']), { kind: 'init', format: 'yaml' });
});

test('parseArgs init accepts --format json', t => {
  t.like(parse.parseArgs(['init', '--format', 'json']), { kind: 'init', format: 'json' });
});

test('parseArgs init accepts --format ts', t => {
  t.like(parse.parseArgs(['init', '--format', 'ts']), { kind: 'init', format: 'ts' });
});

test('parseArgs init accepts --format js', t => {
  t.like(parse.parseArgs(['init', '--format', 'js']), { kind: 'init', format: 'js' });
});

test('parseArgs init rejects unknown --format value', t => {
  const err = t.throws(() => parse.parseArgs(['init', '--format', 'zsh']));
  t.regex(err!.message, /--format/);
  t.regex(err!.message, /yaml.*json.*ts.*js/);
});

test('parseArgs init: --format requires a value', t => {
  const err = t.throws(() => parse.parseArgs(['init', '--format']));
  t.regex(err!.message, /--format requires a value/);
});

test('parseArgs init: --format must immediately precede a non-flag value', t => {
  const err = t.throws(() => parse.parseArgs(['init', '--format', '--help']));
  t.regex(err!.message, /--format requires a value/);
});

test('parseArgs throws for unsupported command', t => {
  const error = t.throws(() => parse.parseArgs(['unknown-cmd']));
  t.true(error?.message.includes('Unsupported command'));
});

test('parseArgs throws for unsupported argument', t => {
  const error = t.throws(() => parse.parseArgs(['generate', '--bogus']));
  t.true(error?.message.includes('Unsupported argument'));
});

test('parseArgs collects --input / --output (and short forms)', t => {
  const long = parse.parseArgs(['generate', '--input', 'a.yaml', '--output', 'out']);
  t.like(long, { kind: 'generate', inputPath: 'a.yaml', outputPath: 'out' });

  const short = parse.parseArgs(['generate', '-i', 'b.yaml', '-o', 'out2']);
  t.like(short, { kind: 'generate', inputPath: 'b.yaml', outputPath: 'out2' });
});

test('parseArgs: --emit accepts comma-separated list', t => {
  const result = parse.parseArgs(['generate', '--emit', 'models,angular']);
  t.deepEqual(result.emit, ['models', 'angular']);
});

test('parseArgs: --emit is repeatable, entries dedupe', t => {
  const result = parse.parseArgs([
    'generate',
    '--emit',
    'models',
    '--emit',
    'angular,models',
  ]);
  t.deepEqual(result.emit, ['models', 'angular']);
});

test('parseArgs: --emit rejects unknown targets at parse time', t => {
  const err = t.throws(() =>
    parse.parseArgs(['generate', '--emit', 'models,react-query']),
  );
  t.true(err?.message.includes("Unknown emit target: 'react-query'"));
});

test('parseArgs: --verbose flag', t => {
  const present = parse.parseArgs(['generate', '--verbose']);
  t.is(present.verbose, true);
  const absent = parse.parseArgs(['generate']);
  t.is(absent.verbose, null);
});

test('parseArgs: emit absent yields null (not empty array)', t => {
  const result = parse.parseArgs(['generate', '--input', 'a.yaml']);
  t.is(result.emit, null);
});

test('parseArgs collects multiple --mapped-type into an array', t => {
  const result = parse.parseArgs([
    'generate',
    '--mapped-type',
    'A:@demo:AType',
    '--mapped-type',
    'B:@demo:BType:BAlias',
  ]);
  t.deepEqual(result.mappedTypes, [
    { schema: 'A', import: '@demo', type: 'AType', alias: undefined },
    { schema: 'B', import: '@demo', type: 'BType', alias: 'BAlias' },
  ]);
});

test('parseArgs: --mapped-type absent yields null (not empty array)', t => {
  const result = parse.parseArgs(['generate', '--input', 'a.yaml']);
  t.is(result.mappedTypes, null);
});

test('parseArgs extracts global --config before command', t => {
  const result = parse.parseArgs([
    '--config',
    '/tmp/cfg.yaml',
    'generate',
    '--input',
    'a.yaml',
  ]);
  t.like(result, { kind: 'generate', configPath: '/tmp/cfg.yaml', inputPath: 'a.yaml' });
});

test('parseArgs extracts global -c after command (interleaved)', t => {
  const result = parse.parseArgs([
    'generate',
    '-c',
    '/tmp/cfg.yaml',
    '--input',
    'a.yaml',
  ]);
  t.like(result, { kind: 'generate', configPath: '/tmp/cfg.yaml', inputPath: 'a.yaml' });
});

// ── Flag value validation (no silent token consumption) ─────────────────────

test('parseArgs: --config errors when next token is another flag', t => {
  const err = t.throws(() => parse.parseArgs(['--config', '--input', 'spec.yaml']));
  t.regex(err!.message, /--config requires a value/);
});

test('parseArgs: -c errors when next token is another flag', t => {
  const err = t.throws(() => parse.parseArgs(['-c', '--input', 'spec.yaml']));
  t.regex(err!.message, /--config requires a value/);
});

test('parseArgs: --input errors when next token is another flag', t => {
  const err = t.throws(() => parse.parseArgs(['generate', '--input', '--output', 'out']));
  t.regex(err!.message, /--input requires a value/);
});

test('parseArgs: -i errors when next token is another flag', t => {
  const err = t.throws(() => parse.parseArgs(['generate', '-i', '--output', 'out']));
  t.regex(err!.message, /--input requires a value/);
});

test('parseArgs: --output errors when next token is missing', t => {
  const err = t.throws(() => parse.parseArgs(['generate', '--output']));
  t.regex(err!.message, /--output requires a value/);
});

test('parseArgs: -o errors when next token starts with --', t => {
  const err = t.throws(() => parse.parseArgs(['generate', '-o', '--verbose']));
  t.regex(err!.message, /--output requires a value/);
});

test('parseArgs: --emit errors when next token is another flag', t => {
  const err = t.throws(() =>
    parse.parseArgs(['generate', '--emit', '--input', 'spec.yaml']),
  );
  t.regex(err!.message, /--emit requires a value/);
});

test('parseArgs: --emit errors when next token is missing', t => {
  const err = t.throws(() => parse.parseArgs(['generate', '--emit']));
  t.regex(err!.message, /--emit requires a value/);
});

test('parseArgs: --mapped-type errors when next token starts with --', t => {
  const err = t.throws(() =>
    parse.parseArgs(['generate', '--mapped-type', '--input', 'spec.yaml']),
  );
  t.regex(err!.message, /--mapped-type requires a value/);
});

test('parseArgs: --mapped-type errors when next token is missing', t => {
  const err = t.throws(() => parse.parseArgs(['generate', '--mapped-type']));
  t.regex(err!.message, /--mapped-type requires a value/);
});

test('mergeConfig forwards file-config naming.methodName to the merged result', t => {
  const merged = parse.mergeConfig(
    { naming: { methodName: '{operationId}' } },
    { kind: 'generate', emit: null, mappedTypes: null },
  );
  t.deepEqual(merged.naming, { methodName: '{operationId}' });
});

test('mergeConfig rejects naming.parse when not a RegExp (preserves YAML/JSON safety)', t => {
  const fileConfig = {
    naming: { methodName: { parse: 'literal-string-from-yaml' } },
  };
  const err = t.throws(() => parse.mergeConfig(fileConfig, {}));
  t.is((err as NodeJS.ErrnoException).code, 'E_INPUT_INVALID');
  t.regex(err!.message, /parse.*RegExp/i);
});

test('mergeConfig accepts RegExp on naming.parse (JS/TS config path)', t => {
  const fileConfig = {
    naming: {
      methodName: {
        from: '{operationId}',
        parse: /^[^_]+_(?<rest>.+)$/,
        format: '{capture.rest}',
        case: 'camel',
      },
    },
  };
  const merged = parse.mergeConfig(fileConfig, {}) as {
    naming?: { methodName?: { parse?: RegExp } };
  };
  t.true(merged.naming?.methodName?.parse instanceof RegExp);
  t.is(merged.naming?.methodName?.parse?.source, '^[^_]+_(?<rest>.+)$');
});

test('mergeConfig rejects non-RegExp parse (e.g. string from YAML)', t => {
  const fileConfig = {
    naming: {
      methodName: {
        from: '{operationId}',
        parse: '^[^_]+_(?<rest>.+)$', // a string, as YAML would deliver
      },
    },
  };
  const err = t.throws(() => parse.mergeConfig(fileConfig, {}));
  t.is((err as NodeJS.ErrnoException).code, 'E_INPUT_INVALID');
  t.regex(err!.message, /parse.*RegExp/i);
});

// ── DEFAULT_EMIT ─────────────────────────────────────────────────────────────

test('DEFAULT_EMIT is models+angular', t => {
  t.deepEqual([...parse.DEFAULT_EMIT], ['models', 'angular']);
});

// ── discoverConfigPath ───────────────────────────────────────────────────────

test('discoverConfigPath returns null when no config exists in tree', async t => {
  await withTempDir(async dir => {
    // tmpdir parents may have a config; isolate by using an even-deeper temp.
    const sub = fs.mkdtempSync(path.join(dir, 'isolated-'));
    // We can only assert null if the result is null OR walks past dir.
    // Make a strict check: if a config IS found, it must not be inside dir.
    const found = parse.discoverConfigPath(sub);
    if (found !== null) {
      t.false(found.startsWith(dir), `unexpected config inside isolated dir: ${found}`);
    } else {
      t.pass();
    }
  });
});

test('discoverConfigPath finds .openapi-ng.yaml in startDir', async t => {
  await withTempDir(async dir => {
    const target = path.join(dir, '.openapi-ng.yaml');
    fs.writeFileSync(target, 'input: a.yaml\n', 'utf8');
    t.is(parse.discoverConfigPath(dir), target);
  });
});

test('discoverConfigPath prefers .yaml over .json when both present', async t => {
  await withTempDir(async dir => {
    fs.writeFileSync(path.join(dir, '.openapi-ng.yaml'), 'input: a\n', 'utf8');
    fs.writeFileSync(path.join(dir, '.openapi-ng.json'), '{"input":"b"}', 'utf8');
    t.is(parse.discoverConfigPath(dir), path.join(dir, '.openapi-ng.yaml'));
  });
});

test('discoverConfigPath falls back to .openapi-ng.json when .yaml absent', async t => {
  await withTempDir(async dir => {
    const target = path.join(dir, '.openapi-ng.json');
    fs.writeFileSync(target, '{"input":"a.yaml"}', 'utf8');
    t.is(parse.discoverConfigPath(dir), target);
  });
});

test('discoverConfigPath walks up the directory tree', async t => {
  await withTempDir(async dir => {
    const target = path.join(dir, '.openapi-ng.yaml');
    fs.writeFileSync(target, 'input: a\n', 'utf8');
    const nested = path.join(dir, 'a', 'b', 'c');
    fs.mkdirSync(nested, { recursive: true });
    t.is(parse.discoverConfigPath(nested), target);
  });
});

test('discoverConfigPath finds openapi-ng.config.ts in startDir', async t => {
  await withTempDir(async dir => {
    const target = path.join(dir, 'openapi-ng.config.ts');
    fs.writeFileSync(target, 'export default {}\n', 'utf8');
    t.is(parse.discoverConfigPath(dir), target);
  });
});

test('discoverConfigPath finds openapi-ng.config.mts in startDir', async t => {
  await withTempDir(async dir => {
    const target = path.join(dir, 'openapi-ng.config.mts');
    fs.writeFileSync(target, 'export default {}\n', 'utf8');
    t.is(parse.discoverConfigPath(dir), target);
  });
});

test('discoverConfigPath finds openapi-ng.config.cts in startDir', async t => {
  await withTempDir(async dir => {
    const target = path.join(dir, 'openapi-ng.config.cts');
    fs.writeFileSync(target, 'module.exports = {}\n', 'utf8');
    t.is(parse.discoverConfigPath(dir), target);
  });
});

test('discoverConfigPath finds openapi-ng.config.mjs in startDir', async t => {
  await withTempDir(async dir => {
    const target = path.join(dir, 'openapi-ng.config.mjs');
    fs.writeFileSync(target, 'export default {}\n', 'utf8');
    t.is(parse.discoverConfigPath(dir), target);
  });
});

test('discoverConfigPath finds openapi-ng.config.js in startDir', async t => {
  await withTempDir(async dir => {
    const target = path.join(dir, 'openapi-ng.config.js');
    fs.writeFileSync(target, 'module.exports = {}\n', 'utf8');
    t.is(parse.discoverConfigPath(dir), target);
  });
});

test('discoverConfigPath finds openapi-ng.config.cjs in startDir', async t => {
  await withTempDir(async dir => {
    const target = path.join(dir, 'openapi-ng.config.cjs');
    fs.writeFileSync(target, 'module.exports = {}\n', 'utf8');
    t.is(parse.discoverConfigPath(dir), target);
  });
});

test('discoverConfigPath prefers openapi-ng.config.ts over plain .js when both present', async t => {
  await withTempDir(async dir => {
    fs.writeFileSync(
      path.join(dir, 'openapi-ng.config.ts'),
      'export default {}\n',
      'utf8',
    );
    fs.writeFileSync(
      path.join(dir, 'openapi-ng.config.js'),
      'module.exports = {}\n',
      'utf8',
    );
    t.is(parse.discoverConfigPath(dir), path.join(dir, 'openapi-ng.config.ts'));
  });
});

test('discoverConfigPath prefers openapi-ng.config.js over legacy .openapi-ng.yaml', async t => {
  await withTempDir(async dir => {
    fs.writeFileSync(
      path.join(dir, 'openapi-ng.config.js'),
      'module.exports = {}\n',
      'utf8',
    );
    fs.writeFileSync(path.join(dir, '.openapi-ng.yaml'), 'input: a\n', 'utf8');
    t.is(parse.discoverConfigPath(dir), path.join(dir, 'openapi-ng.config.js'));
  });
});

test('discoverConfigPath walks up to find openapi-ng.config.ts in parent', async t => {
  await withTempDir(async dir => {
    const target = path.join(dir, 'openapi-ng.config.ts');
    fs.writeFileSync(target, 'export default {}\n', 'utf8');
    const nested = path.join(dir, 'a', 'b', 'c');
    fs.mkdirSync(nested, { recursive: true });
    t.is(parse.discoverConfigPath(nested), target);
  });
});

// ── loadConfigFile ───────────────────────────────────────────────────────────

test('loadConfigFile parses YAML files with array-form emit', async t => {
  await withTempDir(async dir => {
    const p = path.join(dir, '.openapi-ng.yaml');
    fs.writeFileSync(p, 'input: a.yaml\nemit:\n  - models\n  - angular\n', 'utf8');
    t.deepEqual(await parse.loadConfigFile(p), {
      input: 'a.yaml',
      emit: ['models', 'angular'],
    });
  });
});

test('loadConfigFile parses JSON files', async t => {
  await withTempDir(async dir => {
    const p = path.join(dir, '.openapi-ng.json');
    fs.writeFileSync(p, '{"input":"a.yaml","emit":["models","angular"]}', 'utf8');
    t.deepEqual(await parse.loadConfigFile(p), {
      input: 'a.yaml',
      emit: ['models', 'angular'],
    });
  });
});

test('loadConfigFile returns {} for empty YAML', async t => {
  await withTempDir(async dir => {
    const p = path.join(dir, '.openapi-ng.yaml');
    fs.writeFileSync(p, '', 'utf8');
    t.deepEqual(await parse.loadConfigFile(p), {});
  });
});

test('loadConfigFile is case-insensitive on extension (.JSON treated as json)', async t => {
  await withTempDir(async dir => {
    const p = path.join(dir, '.openapi-ng.JSON');
    fs.writeFileSync(p, '{"input":"a.yaml"}', 'utf8');
    t.deepEqual(await parse.loadConfigFile(p), { input: 'a.yaml' });
  });
});

test('loadConfigFile loads .cjs with module.exports = object', async t => {
  await withTempDir(async dir => {
    const p = path.join(dir, 'openapi-ng.config.cjs');
    fs.writeFileSync(p, "module.exports = { input: 'a.yaml', output: 'o' };\n", 'utf8');
    t.deepEqual(await parse.loadConfigFile(p), { input: 'a.yaml', output: 'o' });
  });
});

test('loadConfigFile loads .mjs with export default object', async t => {
  await withTempDir(async dir => {
    const p = path.join(dir, 'openapi-ng.config.mjs');
    fs.writeFileSync(p, "export default { input: 'a.yaml', output: 'o' };\n", 'utf8');
    t.deepEqual(await parse.loadConfigFile(p), { input: 'a.yaml', output: 'o' });
  });
});

test('loadConfigFile loads .js (CJS) with module.exports', async t => {
  await withTempDir(async dir => {
    // A .js file in a tmpdir with no package.json defaults to CJS under
    // Node's resolution rules — `module.exports = ...` is the right form.
    const p = path.join(dir, 'openapi-ng.config.js');
    fs.writeFileSync(p, "module.exports = { input: 'a.yaml', output: 'o' };\n", 'utf8');
    t.deepEqual(await parse.loadConfigFile(p), { input: 'a.yaml', output: 'o' });
  });
});

test('loadConfigFile awaits an async-function default export', async t => {
  await withTempDir(async dir => {
    const p = path.join(dir, 'openapi-ng.config.mjs');
    fs.writeFileSync(
      p,
      "export default async () => ({ input: 'a.yaml', output: 'o' });\n",
      'utf8',
    );
    t.deepEqual(await parse.loadConfigFile(p), { input: 'a.yaml', output: 'o' });
  });
});

test('loadConfigFile rejects a JS file with no default export', async t => {
  await withTempDir(async dir => {
    const p = path.join(dir, 'openapi-ng.config.mjs');
    // No default export — just a named export.
    fs.writeFileSync(p, 'export const x = 1;\n', 'utf8');
    const err = await t.throwsAsync(() => parse.loadConfigFile(p));
    t.is((err as NodeJS.ErrnoException).code, 'E_INPUT_INVALID');
    t.regex(err!.message, /no default export/i);
  });
});

test('loadConfigFile rejects a JS file whose default export is null', async t => {
  await withTempDir(async dir => {
    const p = path.join(dir, 'openapi-ng.config.mjs');
    fs.writeFileSync(p, 'export default null;\n', 'utf8');
    const err = await t.throwsAsync(() => parse.loadConfigFile(p));
    t.is((err as NodeJS.ErrnoException).code, 'E_INPUT_INVALID');
    t.regex(err!.message, /default export must be an object/i);
  });
});

test('loadConfigFile rejects a JS file whose default export is a primitive', async t => {
  await withTempDir(async dir => {
    const p = path.join(dir, 'openapi-ng.config.mjs');
    fs.writeFileSync(p, 'export default 42;\n', 'utf8');
    const err = await t.throwsAsync(() => parse.loadConfigFile(p));
    t.is((err as NodeJS.ErrnoException).code, 'E_INPUT_INVALID');
    t.regex(err!.message, /default export must be an object/i);
  });
});

test('loadConfigFile rejects a JS file whose default export is an array', async t => {
  await withTempDir(async dir => {
    const p = path.join(dir, 'openapi-ng.config.mjs');
    fs.writeFileSync(p, "export default ['models'];\n", 'utf8');
    const err = await t.throwsAsync(() => parse.loadConfigFile(p));
    t.is((err as NodeJS.ErrnoException).code, 'E_INPUT_INVALID');
    t.regex(err!.message, /default export must be an object/i);
  });
});

test('loadConfigFile wraps module-load errors as E_INPUT_INVALID', async t => {
  await withTempDir(async dir => {
    const p = path.join(dir, 'openapi-ng.config.mjs');
    // Syntax error at module top — Node's loader throws during import.
    fs.writeFileSync(p, 'export default { broken\n', 'utf8');
    const err = await t.throwsAsync(() => parse.loadConfigFile(p));
    t.is((err as NodeJS.ErrnoException).code, 'E_INPUT_INVALID');
    t.regex(err!.message, /Failed to load config file/i);
  });
});

// On Windows + Node 20 the @oxc-node/core/register hook fails inside its
// own native binding with "Missing field `format`" before our mapping
// branch ever sees ERR_UNKNOWN_FILE_EXTENSION, so the canonical two-outcome
// contract this test guards doesn't apply on that combo. The same loader
// works fine on Linux/macOS Node 20 and on Windows Node 22+, so skipping
// only the affected matrix entry preserves coverage everywhere else.
const skipCtsLoaderTest =
  process.platform === 'win32' && Number(process.versions.node.split('.')[0]) < 22;
(skipCtsLoaderTest ? test.skip : test)(
  'loadConfigFile maps ERR_UNKNOWN_FILE_EXTENSION on .cts to a version hint',
  async t => {
    // Two-outcome contract: on Node ≥ 22.6 native TS stripping handles
    // the .cts file and the load succeeds; on Node < 22.6 the loader
    // throws ERR_UNKNOWN_FILE_EXTENSION and our mapping branch turns it
    // into a friendly E_INPUT_INVALID. Inside the AVA worker the
    // @oxc-node/core/register hook handles TS files itself, so the
    // success branch fires; in production with a vanilla Node binary
    // the branch chosen depends on the running version. Either outcome
    // proves we never surface a generic "Failed to load" wrap for a
    // .cts file when the underlying error is ERR_UNKNOWN_FILE_EXTENSION.
    await withTempDir(async dir => {
      const p = path.join(dir, 'openapi-ng.config.cts');
      fs.writeFileSync(
        p,
        "const cfg: { input: string } = { input: 'a.yaml' };\nmodule.exports = cfg;\n",
        'utf8',
      );
      try {
        const result = await parse.loadConfigFile(p);
        // Modern Node — strip-types worked.
        t.deepEqual(result, { input: 'a.yaml' });
      } catch (err) {
        const e = err as NodeJS.ErrnoException;
        t.is(e.code, 'E_INPUT_INVALID');
        t.regex(e.message, /TypeScript config files require Node 22\.6\+/i);
        t.regex(e.message, /--experimental-strip-types|23\.6/i);
      }
    });
  },
);

// Smoke test: only meaningful on Node ≥ 22.6 where native TS stripping
// is available. Skipped otherwise so old-Node CI runners stay green.
// Uses .mts with `export default` syntax — strip-friendly because
// Node's --experimental-strip-types only removes type annotations and
// leaves valid ESM; no transpilation of TS-specific syntax needed.
const [nodeMajor, nodeMinor] = process.versions.node.split('.').map(Number);
const tsNativeAvailable = nodeMajor > 22 || (nodeMajor === 22 && nodeMinor >= 6);

(tsNativeAvailable ? test : test.skip)(
  'loadConfigFile loads .mts with native TS stripping (Node ≥ 22.6)',
  async t => {
    await withTempDir(async dir => {
      const p = path.join(dir, 'openapi-ng.config.mts');
      fs.writeFileSync(
        p,
        "interface C { input: string }\nexport default { input: 'a.yaml' } satisfies C;\n",
        'utf8',
      );
      t.deepEqual(await parse.loadConfigFile(p), { input: 'a.yaml' });
    });
  },
);
