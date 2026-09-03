#!/usr/bin/env node
// Post-processes the NAPI-RS-generated `index.d.ts`:
//   1. Narrows `code: string` to `code: DiagnosticCode` (named union)
//      and `severity: string` to `severity: 'warning' | 'error'` so TS
//      consumers get autocomplete and exhaustive switches.
//   2. Rewrites the `export declare const enum EmitTarget` block into a
//      string-literal union plus an ambient frozen-const declaration.
//      `const enum` in a published `.d.ts` is hostile to consumers
//      compiling under `isolatedModules` / `verbatimModuleSyntax` (Vite,
//      esbuild, Bun, TS 5.x defaults), so we replace the shape with one
//      that survives single-file transpilation.
//   3. Concatenates the hand-authored tail file `index.d.ts.in`, which
//      carries the `DiagnosticCode` union and the `GenerateError` class
//      declaration (defined in `lib/index.js`, not by napi-rs).
//
// Run automatically by the `postbuild` npm script after `napi build`.

import fs from 'node:fs';
import path from 'node:path';
import url from 'node:url';

const __dirname = path.dirname(url.fileURLToPath(import.meta.url));
const repoRoot = path.join(__dirname, '..');
const dtsPath = path.join(repoRoot, 'index.d.ts');
const tailPath = path.join(repoRoot, 'index.d.ts.in');

let content = fs.readFileSync(dtsPath, 'utf8');
const tail = fs.readFileSync(tailPath, 'utf8');

// Strip any prior tail concatenation so reruns stay idempotent.
const TAIL_BEGIN = '\n// Hand-authored tail';
const beginIdx = content.indexOf(TAIL_BEGIN);
if (beginIdx !== -1) {
  content = content.slice(0, beginIdx).trimEnd() + '\n';
}

/**
 * Scope a set of literal text substitutions to the body of a named
 * interface declaration. Catches drift (e.g. napi-rs changing indent,
 * or another interface coincidentally containing `code: string`) early
 * by failing loud when a target cannot be matched within the named
 * block.
 */
function patchInterface(src, name, edits) {
  const re = new RegExp(`(interface\\s+${name}\\s*\\{)([\\s\\S]*?)(^})`, 'm');
  const m = src.match(re);
  if (!m) throw new Error(`interface ${name} not found`);
  let body = m[2];
  for (const [from, to] of edits) {
    if (!body.includes(from)) {
      // Idempotent: already narrowed by a prior run.
      if (body.includes(to)) continue;
      throw new Error(`patch target not found in ${name}: ${from}`);
    }
    body = body.split(from).join(to);
  }
  return src.replace(re, `$1${body}$3`);
}

// Narrow the napi-emitted opaque strings to typed unions. Same shape
// (`code: string`, `severity: string`) appears in multiple interfaces
// (GeneratorDiagnostic, GenerateErrorPayload) — we scope each set of
// substitutions to its own interface block so a stray `code: string`
// elsewhere can never silently corrupt the patch.
const diagnosticEdits = [
  ['  code: string', '  code: DiagnosticCode'],
  ['  subcode?: string', '  subcode: DiagnosticSubcode | null'],
  ['  severity: string', "  severity: 'warning' | 'error'"],
];
const payloadEdits = [
  ['  code: string', '  code: DiagnosticCode'],
  ['  subcode?: string', '  subcode: DiagnosticSubcode | null'],
];

content = patchInterface(content, 'GeneratorDiagnostic', diagnosticEdits);
content = patchInterface(content, 'GenerateErrorPayload', payloadEdits);

// Replace the napi-rs-emitted `const enum EmitTarget` with a string-literal
// union plus an ambient frozen-const declaration. Consumers compiling under
// `isolatedModules` reject `const enum` imports across module boundaries
// (Vite, esbuild, Bun, TS 5.x defaults); the union+const pair preserves the
// `EmitTarget.Models` access shape while keeping the surface importable.
const ENUM_BLOCK_RE =
  /export declare const enum EmitTarget \{\s*Models = 'models',\s*Angular = 'angular'\s*\}/;
const ENUM_REPLACEMENT =
  "export type EmitTarget = 'models' | 'angular';\n" +
  'export declare const EmitTarget: {\n' +
  "  readonly Models: 'models';\n" +
  "  readonly Angular: 'angular';\n" +
  '};';
if (ENUM_BLOCK_RE.test(content)) {
  content = content.replace(ENUM_BLOCK_RE, ENUM_REPLACEMENT);
} else if (!content.includes("export type EmitTarget = 'models' | 'angular';")) {
  throw new Error(
    'EmitTarget const-enum block not found and union form not already present',
  );
}

// Mark `GenerateOptions.emit` optional on the published surface. The JS
// wrapper in `lib/index.js` defaults the field to `['models', 'angular']`
// (mirroring the CLI's DEFAULT_EMIT in `bin/lib/parse.js`) before
// crossing the NAPI boundary, so consumers can omit it. Kept as a
// rewrite here — rather than annotating the Rust field as
// `Option<Vec<EmitTarget>>` — so the core `GenerateConfig::from`
// conversion (and its cargo-side tests) continue to receive a
// populated emit list with no extra None-handling.
if (content.includes('emit: Array<EmitTarget>')) {
  content = content.replace('emit: Array<EmitTarget>', 'emit?: Array<EmitTarget>');
} else if (!content.includes('emit?: Array<EmitTarget>')) {
  throw new Error('patch-types: expected `emit: Array<EmitTarget>` in index.d.ts');
}

// Rewrite `GenerateOptions.naming` from the NAPI-boundary shape
// (`NamingOptions`) to the user-friendly shape (`NamingConfig`).
// Native `RegExp` does not cross the NAPI boundary as a JS object so
// the Rust side declares `NamingParseSpec { source, flags }`; the JS
// wrapper unpacks user `RegExp`s into that shape. Consumers should
// only ever see the friendly `NamingConfig` type on the published
// surface, not the wire shape.
if (content.includes('naming?: NamingOptions')) {
  content = content.replace('naming?: NamingOptions', 'naming?: NamingConfig');
} else if (!content.includes('naming?: NamingConfig')) {
  throw new Error('patch-types: expected `naming?: NamingOptions` in index.d.ts');
}

// The native export and its union are wrapper-internal. Remove them (and
// their leading doc comment) from the public surface; the hand-authored
// tail declares `generate`.
// `[^*]|\*(?!/)` (not `[\s\S]*?`) so the lazy match can't skip past this
// declaration's own closing `*/` and swallow a later, unrelated block.
const LEADING_DOC = '(?:^/\\*\\*(?:[^*]|\\*(?!/))*\\*/\\n)?';
const NATIVE_FN_RE = new RegExp(
  `${LEADING_DOC}^export declare function generateNative\\([^\\n]*\\n`,
  'm',
);
const OUTCOME_RE = new RegExp(
  `${LEADING_DOC}^export interface GenerateOutcome \\{[\\s\\S]*?^\\}\\n`,
  'm',
);
content = content.replace(NATIVE_FN_RE, '').replace(OUTCOME_RE, '');
// Scoped to the declaration forms because GenerateErrorPayload's doc comment
// legitimately mentions `GenerateOutcome.error` in prose.
if (
  /^export (?:declare function generateNative|interface GenerateOutcome)\b/m.test(content)
) {
  throw new Error(
    'patch-types: generateNative/GenerateOutcome declaration survived stripping; update the patterns',
  );
}

// `inputPath` becomes optional on the published surface because the
// caller may instead pass `inputContents` (validated mutually
// exclusive at runtime). napi-rs may emit it as either required or
// optional depending on whether the Rust field is Option<String> at
// build time; both cases are handled here.
if (content.includes('inputPath: string')) {
  content = content.replace('inputPath: string', 'inputPath?: string');
} else if (!content.includes('inputPath?: string')) {
  throw new Error(
    'patch-types: expected `inputPath: string` or `inputPath?: string` in index.d.ts',
  );
}

content = content.trimEnd() + '\n\n' + tail.trimEnd() + '\n';
// Stripped declarations leave runs of blank lines behind.
content = content.replace(/\n{3,}/g, '\n\n');

fs.writeFileSync(dtsPath, content);
console.log('patch-types: narrowed code/severity and appended tail to index.d.ts');

// ---------------------------------------------------------------------------
// Patch native.js: inject a friendly error for unsupported platforms BEFORE
// the generic npm-bug-report throw that NAPI-RS emits.
// ---------------------------------------------------------------------------
const nativePath = path.join(repoRoot, 'native.js');
let nativeContent = fs.readFileSync(nativePath, 'utf8');

// Idempotency guard: skip if the injection is already present.
const INJECTION_GUARD = '__OPENAPI_NG_PLATFORM_KEY__';
if (!nativeContent.includes(INJECTION_GUARD)) {
  // The exact marker string that NAPI-RS emits; fail loud if it drifts.
  const MARKER = 'if (!nativeBinding) {\n  if (loadErrors.length > 0) {';
  const markerIdx = nativeContent.indexOf(MARKER);
  if (markerIdx === -1) {
    throw new Error(
      'patch-native-loader: cannot find expected marker in native.js — ' +
        'NAPI-RS may have changed its generated output; update the patch.',
    );
  }

  const injection =
    "const __OPENAPI_NG_PLATFORM_KEY__ = process.platform + '/' + process.arch;\n" +
    'const __OPENAPI_NG_SUPPORTED__ = new Set([\n' +
    "  'darwin/x64', 'darwin/arm64',\n" +
    "  'linux/x64', 'linux/arm64',\n" +
    "  'win32/x64', 'win32/arm64',\n" +
    ']);\n' +
    'if (!nativeBinding && !__OPENAPI_NG_SUPPORTED__.has(__OPENAPI_NG_PLATFORM_KEY__)) {\n' +
    '  throw new Error(\n' +
    "    'openapi-ng does not ship a native binary for ' + __OPENAPI_NG_PLATFORM_KEY__ + '. ' +\n" +
    "    'Supported platforms: ' + [...__OPENAPI_NG_SUPPORTED__].sort().join(', ') + '. ' +\n" +
    "    'If you need this platform, please file an issue, or install @avsystem/openapi-ng-wasm32-wasi for a WebAssembly fallback.',\n" +
    '  );\n' +
    '}\n\n';

  nativeContent =
    nativeContent.slice(0, markerIdx) + injection + nativeContent.slice(markerIdx);

  fs.writeFileSync(nativePath, nativeContent);
  console.log('patch-types: injected unsupported-platform error into native.js');
} else {
  console.log('patch-types: native.js already patched (idempotent skip)');
}

// ---------------------------------------------------------------------------
// Re-author browser.js. `napi build` writes an `export *` stub over it, so
// the post-build step restores the real entry, which lives in
// lib/browser.js where napi never touches it.
// ---------------------------------------------------------------------------
const browserPath = path.join(repoRoot, 'browser.js');
const browserContent = `'use strict';\n\nmodule.exports = require('./lib/browser.js');\n`;

const existingBrowser = fs.existsSync(browserPath)
  ? fs.readFileSync(browserPath, 'utf8')
  : null;
if (existingBrowser === browserContent) {
  console.log('patch-types: browser.js already canonical (idempotent skip)');
} else {
  fs.writeFileSync(browserPath, browserContent);
  console.log('patch-types: re-authored browser.js as a lib/browser.js re-export');
}

// `browser.d.ts` types the `./browser` subpath. napi never writes a file with
// this name, so it is hand-authored and committed; fail loud if it goes
// missing rather than publishing an untyped browser entry.
const browserDtsPath = path.join(repoRoot, 'browser.d.ts');
if (!fs.existsSync(browserDtsPath)) {
  throw new Error(
    'patch-types: browser.d.ts is missing — the ./browser subpath would publish untyped',
  );
}
