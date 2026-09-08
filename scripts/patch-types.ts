#!/usr/bin/env bun
// Post-processes what `napi build` generates, so the published surface is
// the one consumers should see.
//
// A patch that no longer matches fails the build naming itself, so a
// change in NAPI-RS output cannot publish an unpatched surface. Every
// patch is idempotent: a rerun on a patched tree is a no-op.
//
// Runs from the `postbuild` / `postbuild:debug` scripts.

import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), '..');
const dtsPath = join(repoRoot, 'index.d.ts');
const tailPath = join(repoRoot, 'index.d.ts.in');
const nativePath = join(repoRoot, 'native.js');
const browserPath = join(repoRoot, 'browser.js');
const browserDtsPath = join(repoRoot, 'browser.d.ts');

/** One rewrite of a generated file. */
interface Patch {
  /** Named in the drift error when the patch cannot be applied. */
  readonly name: string;
  /**
   * Returns the rewritten source, or `null` when the source is already in
   * the target shape. Throws when it is in neither.
   */
  readonly apply: (source: string) => string | null;
}

class DriftError extends Error {
  constructor(patch: string, detail: string) {
    super(
      `patch-types: ${patch} could not be applied — ${detail}. ` +
        `NAPI-RS output may have changed; update the patch.`,
    );
    this.name = 'DriftError';
  }
}

function applyAll(source: string, patches: readonly Patch[]): string {
  return patches.reduce((current, patch) => patch.apply(current) ?? current, source);
}

/**
 * Replaces `find` with `replace`. Treats a source that already contains
 * `replace` as already patched; anything else is drift.
 */
function rewrite(name: string, find: string, replace: string): Patch {
  return {
    name,
    apply: source => {
      if (source.includes(find)) return source.split(find).join(replace);
      if (source.includes(replace)) return null;
      throw new DriftError(name, `neither ${JSON.stringify(find)} nor its replacement found`);
    },
  };
}

/** Replaces the first match of `find`, or does nothing when `settled` holds. */
function rewritePattern(
  name: string,
  find: RegExp,
  replace: string,
  settled: (source: string) => boolean,
): Patch {
  return {
    name,
    apply: source => {
      if (find.test(source)) return source.replace(find, replace);
      if (settled(source)) return null;
      throw new DriftError(name, `pattern ${find} did not match`);
    },
  };
}

/**
 * Scopes literal substitutions to the body of one named interface, so a
 * coincidental `code: string` elsewhere can never be caught by the patch.
 */
function withinInterface(
  name: string,
  interfaceName: string,
  edits: readonly (readonly [string, string])[],
): Patch {
  /** Capture group holding the interface body, between its braces. */
  const BODY_GROUP = 2;
  const block = new RegExp(`(interface\\s+${interfaceName}\\s*\\{)([\\s\\S]*?)(^})`, 'm');
  return {
    name,
    apply: source => {
      const found = source.match(block);
      if (!found) throw new DriftError(name, `interface ${interfaceName} not found`);
      let body = found[BODY_GROUP];
      if (body === undefined) {
        throw new DriftError(name, 'the interface-body capture group is missing');
      }
      for (const [from, to] of edits) {
        if (body.includes(from)) {
          body = body.split(from).join(to);
        } else if (!body.includes(to)) {
          throw new DriftError(name, `${JSON.stringify(from)} not found in ${interfaceName}`);
        }
      }
      return source.replace(block, `$1${body}$3`);
    },
  };
}

// Narrow the opaque strings NAPI emits to the named unions consumers can
// switch on exhaustively. The same field shapes appear in more than one
// interface, so each set is scoped to its own block.
const NARROWED_DIAGNOSTIC = [
  ['  code: string', '  code: DiagnosticCode'],
  ['  subcode?: string', '  subcode: DiagnosticSubcode | null'],
  ['  severity: string', "  severity: 'warning' | 'error'"],
] as const;

const EMIT_TARGET_UNION = "export type EmitTarget = 'models' | 'angular';";
const INPUT_FORMAT_UNION = "export type InputFormat = 'json' | 'yaml';";
const RESPONSE_TYPE_UNION =
  "export type ResponseType = 'json' | 'blob' | 'text' | 'arrayBuffer';";

// `[^*]|\*(?!/)` rather than `[\s\S]*?` so a lazy match cannot run past this
// declaration's own `*/` and swallow the next block.
const LEADING_DOC = '(?:^/\\*\\*(?:[^*]|\\*(?!/))*\\*/\\n)?';

const dtsPatches: readonly Patch[] = [
  withinInterface('diagnostic narrowing', 'GeneratorDiagnostic', NARROWED_DIAGNOSTIC),
  withinInterface('error-payload narrowing', 'GenerateErrorPayload', NARROWED_DIAGNOSTIC.slice(0, 2)),

  // A `const enum` in a published .d.ts breaks consumers compiling under
  // isolatedModules / verbatimModuleSyntax (Vite, esbuild, Bun, TS 5+
  // defaults). The union plus an ambient const keeps `EmitTarget.Models`
  // working while staying importable from a single-file transpile.
  rewritePattern(
    'EmitTarget const-enum removal',
    /export declare const enum EmitTarget \{\s*Models = 'models',\s*Angular = 'angular'\s*\}/,
    [
      EMIT_TARGET_UNION,
      'export declare const EmitTarget: {',
      "  readonly Models: 'models';",
      "  readonly Angular: 'angular';",
      '};',
    ].join('\n'),
    source => source.includes(EMIT_TARGET_UNION),
  ),

  // `InputFormat` carries the same const-enum problem as `EmitTarget`.
  rewritePattern(
    'InputFormat const-enum removal',
    /export declare const enum InputFormat \{\s*Json = 'json',\s*Yaml = 'yaml'\s*\}/,
    [
      INPUT_FORMAT_UNION,
      'export declare const InputFormat: {',
      "  readonly Json: 'json';",
      "  readonly Yaml: 'yaml';",
      '};',
    ].join('\n'),
    source => source.includes(INPUT_FORMAT_UNION),
  ),

  rewritePattern(
    'ResponseType const-enum removal',
    /export declare const enum ResponseType \{\s*Json = 'json',\s*Blob = 'blob',\s*Text = 'text',\s*ArrayBuffer = 'arrayBuffer'\s*\}/,
    [
      RESPONSE_TYPE_UNION,
      'export declare const ResponseType: {',
      "  readonly Json: 'json';",
      "  readonly Blob: 'blob';",
      "  readonly Text: 'text';",
      "  readonly ArrayBuffer: 'arrayBuffer';",
      '};',
    ].join('\n'),
    source => source.includes(RESPONSE_TYPE_UNION),
  ),

  // The wrapper defaults `emit` before the boundary, so a consumer may
  // omit it.
  rewrite('optional emit', 'emit: Array<EmitTarget>', 'emit?: Array<EmitTarget>'),

  // `inputPath` is optional because a caller may pass `inputContents`
  // instead; the two are validated mutually exclusive at runtime.
  rewrite('optional inputPath', 'inputPath: string', 'inputPath?: string'),

  // A JS `RegExp` cannot cross the NAPI boundary, so Rust declares the
  // `{ source, flags }` wire shape the wrapper unpacks into.
  rewrite('friendly naming type', 'naming?: NamingOptions', 'naming?: NamingConfig'),

  // The native export and its result union are wrapper-internal; the
  // hand-authored tail declares `generate` instead.
  {
    name: 'native-export stripping',
    apply: source => {
      const stripped = source
        .replace(new RegExp(`${LEADING_DOC}^export declare function generateNative\\([^\\n]*\\n`, 'm'), '')
        .replace(new RegExp(`${LEADING_DOC}^export interface GenerateOutcome \\{[\\s\\S]*?^\\}\\n`, 'm'), '');
      // Scoped to the declaration forms: GenerateErrorPayload's own doc
      // comment legitimately mentions `GenerateOutcome.error` in prose.
      if (/^export (?:declare function generateNative|interface GenerateOutcome)\b/m.test(stripped)) {
        throw new DriftError('native-export stripping', 'a declaration survived');
      }
      return stripped;
    },
  },
];

/** Marks where the hand-authored tail begins, so reruns stay idempotent. */
const TAIL_MARKER = '\n// Hand-authored tail';

function patchTypes(): void {
  const source = readFileSync(dtsPath, 'utf8');
  const priorTail = source.indexOf(TAIL_MARKER);
  const generated = priorTail === -1 ? source : `${source.slice(0, priorTail).trimEnd()}\n`;

  const tail = readFileSync(tailPath, 'utf8').trimEnd();
  const patched = `${applyAll(generated, dtsPatches).trimEnd()}\n\n${tail}\n`;

  // Stripped declarations leave runs of blank lines behind.
  writeFileSync(dtsPath, patched.replace(/\n{3,}/g, '\n\n'));
  console.log('patch-types: narrowed the diagnostic surface and appended the tail to index.d.ts');
}

/** Present once the platform guard has been injected. */
const PLATFORM_GUARD = '__OPENAPI_NG_PLATFORM_KEY__';

/** The exact text NAPI-RS emits before its generic npm-bug-report throw. */
const NAPI_FALLBACK_MARKER = 'if (!nativeBinding) {\n  if (loadErrors.length > 0) {';

/** Grouped by OS, matching how the emitted `native.js` reads. */
const SUPPORTED_PLATFORMS = [
  ["'darwin/x64'", "'darwin/arm64'"],
  ["'linux/x64'", "'linux/arm64'"],
  ["'win32/x64'", "'win32/arm64'"],
];

/**
 * Injects a platform-specific load error ahead of NAPI-RS's generic one, so
 * a consumer on an unsupported platform is told which platforms ship a
 * binary and that a WebAssembly fallback exists.
 */
function patchNativeLoader(): void {
  const source = readFileSync(nativePath, 'utf8');
  if (source.includes(PLATFORM_GUARD)) {
    console.log('patch-types: native.js already patched');
    return;
  }

  const marker = source.indexOf(NAPI_FALLBACK_MARKER);
  if (marker === -1) {
    throw new DriftError('native loader guard', 'the NAPI-RS fallback marker is missing');
  }

  const guard = [
    `const ${PLATFORM_GUARD} = process.platform + '/' + process.arch;`,
    'const __OPENAPI_NG_SUPPORTED__ = new Set([',
    ...SUPPORTED_PLATFORMS.map(group => `  ${group.join(', ')},`),
    ']);',
    `if (!nativeBinding && !__OPENAPI_NG_SUPPORTED__.has(${PLATFORM_GUARD})) {`,
    '  throw new Error(',
    `    'openapi-ng does not ship a native binary for ' + ${PLATFORM_GUARD} + '. ' +`,
    "    'Supported platforms: ' + [...__OPENAPI_NG_SUPPORTED__].sort().join(', ') + '. ' +",
    "    'If you need this platform, please file an issue, or install @avsystem/openapi-ng-wasm32-wasi for a WebAssembly fallback.',",
    '  );',
    '}',
    '',
    '',
  ].join('\n');

  writeFileSync(nativePath, source.slice(0, marker) + guard + source.slice(marker));
  console.log('patch-types: injected the unsupported-platform error into native.js');
}

/** `napi build` overwrites browser.js with an `export *` stub. */
const BROWSER_ENTRY = "'use strict';\n\nmodule.exports = require('./lib/browser.js');\n";

function restoreBrowserEntry(): void {
  const current = existsSync(browserPath) ? readFileSync(browserPath, 'utf8') : null;
  if (current === BROWSER_ENTRY) {
    console.log('patch-types: browser.js already canonical');
    return;
  }
  writeFileSync(browserPath, BROWSER_ENTRY);
  console.log('patch-types: re-authored browser.js as a lib/browser.js re-export');
}

/** `browser.d.ts` is hand-authored: napi does not recreate it. */
function assertBrowserTypesPresent(): void {
  if (!existsSync(browserDtsPath)) {
    throw new Error('patch-types: browser.d.ts is missing — ./browser would publish untyped');
  }
}

patchTypes();
patchNativeLoader();
restoreBrowserEntry();
assertBrowserTypesPresent();
