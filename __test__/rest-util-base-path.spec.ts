// Mirror-test of the URL-join logic used inside `templates/angular/rest.util.ts`
// to prepend `OPENAPI_NG_BASE_PATH` to `CommonRequest.url`. We can't import the
// template directly here — it pulls in `@angular/common/http`, which needs
// Angular's JIT/platform bootstrapping that's heavy to stand up under plain
// Node — so we keep a parallel implementation pinned to the template's
// behaviour. The snapshot suite is the authoritative check that the template
// itself still contains the matching algorithm.
import test from 'ava';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

function joinBasePath(base: string, url: string): string {
  // Absolute URLs (https://…, etc.) bypass the configured basePath.
  if (/^[a-z][a-z0-9+.-]*:\/\//i.test(url)) return url;
  const normalizedBase = base.endsWith('/') ? base.slice(0, -1) : base;
  const normalizedUrl = url.startsWith('/') ? url : `/${url}`;
  return normalizedBase + normalizedUrl;
}

test('joinBasePath: absolute base + leading-slash path', t => {
  t.is(
    joinBasePath('https://api.example.com', '/pets'),
    'https://api.example.com/pets',
  );
});

test('joinBasePath: trailing slash on base is normalised', t => {
  t.is(
    joinBasePath('https://api.example.com/', '/pets'),
    'https://api.example.com/pets',
  );
});

test('joinBasePath: base with subpath', t => {
  t.is(
    joinBasePath('https://api.example.com/v1', '/pets/123'),
    'https://api.example.com/v1/pets/123',
  );
});

test('joinBasePath: relative base + leading-slash path', t => {
  t.is(joinBasePath('/api', '/pets'), '/api/pets');
});

test('joinBasePath: url without leading slash gets one added', t => {
  t.is(joinBasePath('/api', 'pets'), '/api/pets');
});

test('joinBasePath: both trailing- and leading-slash collapse to one slash', t => {
  t.is(joinBasePath('/api/', '/pets'), '/api/pets');
});

test('joinBasePath: absolute http url on request bypasses basePath', t => {
  t.is(
    joinBasePath('https://api.example.com', 'http://other.example.com/pets'),
    'http://other.example.com/pets',
  );
});

test('joinBasePath: absolute https url on request bypasses basePath', t => {
  t.is(
    joinBasePath('/api', 'https://other.example.com/pets'),
    'https://other.example.com/pets',
  );
});

test('joinBasePath: protocol-relative urls are NOT bypassed', t => {
  // `//host/path` is protocol-relative, not absolute by RFC 3986 scheme rules;
  // the regex requires `scheme:` before `//`, so basePath still applies.
  t.is(joinBasePath('/api', '//other.example.com/pets'), '/api//other.example.com/pets');
});

// Pin the template against the parallel implementation: if the template's
// joinBasePath is ever edited away from this algorithm, this test surfaces
// the drift immediately (without booting Angular).
test('template `joinBasePath` source matches this parallel implementation', t => {
  const templatePath = path.resolve(
    __dirname,
    '..',
    'templates',
    'angular',
    'rest.util.ts',
  );
  const source = fs.readFileSync(templatePath, 'utf8');
  const match = source.match(
    /function joinBasePath\(base: string, url: string\): string \{\s*([\s\S]*?)\n\}/u,
  );
  t.truthy(match, 'expected to find joinBasePath in template');
  const body = (match![1] ?? '').replace(/\s+/gu, ' ').trim();
  t.is(
    body,
    "// Absolute URLs (https://…, etc.) bypass the configured basePath. if (/^[a-z][a-z0-9+.-]*:\\/\\//i.test(url)) return url; const normalizedBase = base.endsWith('/') ? base.slice(0, -1) : base; const normalizedUrl = url.startsWith('/') ? url : `/${url}`; return normalizedBase + normalizedUrl;",
    'template joinBasePath drifted from the parallel test implementation',
  );
});
