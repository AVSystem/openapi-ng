---
title: Configuration
description: How openapi-ng discovers config files, the JS/TS config format, mapped types, and naming customization.
---

## Discovery

`openapi-ng` looks for a config file in the working directory and each
parent directory until it finds one. Supported formats, in priority
order:

- `openapi-ng.config.ts` / `.mts` / `.cts` (TypeScript — requires Node 22.6+;
  prefer `.mts` to avoid the CJS-first parse penalty under a typeless
  `package.json`)
- `openapi-ng.config.mjs` / `.js` / `.cjs`
- `.openapi-ng.yaml` (legacy dotfile, parsed as YAML)
- `.openapi-ng.json` (legacy dotfile, parsed as JSON)

A few naming rules worth knowing:

- **Discovery is case-sensitive on Linux.** Keep the filename lowercase
  or it will not be found on Linux.
- **macOS and Windows** typically run on case-insensitive volumes, so a
  mixed-case file may be discovered there but not on Linux. Keep the
  name lowercase to stay portable.

Pass `--config <path>` (or `-c <path>`) to point at a file explicitly
and skip discovery.

Config files are loaded from the local filesystem; do not point
`--config` at an untrusted YAML or JS/TS file. JS/TS configs execute
arbitrary code at load time, and even the YAML parser, while
safe-by-default today, sits inside an implicit trust boundary worth
respecting.

## JS/TS configs

A `openapi-ng.config.ts` (or `.js`/`.mjs`/`.cjs`) lets you use
JavaScript features the YAML/JSON formats cannot express — most
importantly, real `RegExp` literals in `naming.parse`:

```ts
import { defineConfig } from '@avsystem/openapi-ng/config';

export default defineConfig({
  input: './petstore.openapi.yaml',
  output: './src/generated',
  emit: ['models', 'angular'],
  mappedTypes: [
    { schema: 'DateTime', import: 'dayjs', type: 'Dayjs' },
  ],
  naming: {
    methodName: {
      from: '{operationId}',
      parse: /^[^_]+_(?<rest>.+)$/,
      format: '{capture.rest}',
      case: 'camel',
    },
    group: [{ format: '{tags[0]}', case: 'pascal' }],
  },
});
```

The default export may be an object or an async function returning one.
`defineConfig` is an identity helper for TypeScript inference; using it
is optional.

TypeScript files (`.ts`/`.mts`/`.cts`) require Node 22.6+ (or 23.6+ for
the flag-free default). On older Node, use `.js`/`.mjs` with JSDoc
types: `/** @type {import('@avsystem/openapi-ng').Config} */`.

Run `openapi-ng init --format ts` (or `json`/`js`) to scaffold a
starter file.

## Mapped types

Some real-world types are awkward to express in OpenAPI Schema —
think GeoJSON `Feature`/`FeatureCollection`, recursive AST nodes,
or types that already live in a well-typed npm package you'd rather
re-use than re-describe.

Mapped types let you ship a *placeholder* schema in the spec and
swap it for an external TypeScript type at generation time. For
GeoJSON, instead of trying (and failing) to translate the full
GeoJSON RFC into nested `oneOf`/discriminator schemas, you declare
a stub:

```yaml
# petstore.openapi.yaml
components:
  schemas:
    GeoFeature:
      type: object   # placeholder — replaced via mapped type
```

…then point the generator at the real type:

```bash
openapi-ng generate \
  --input petstore.openapi.yaml \
  --output ./generated \
  --mapped-type GeoFeature:geojson:Feature
```

The generated `model.generated.ts` now imports the real type and uses
it everywhere `GeoFeature` was referenced:

```ts
import type { Feature } from 'geojson';

export type GeoFeature = Feature;

// ...wherever GeoFeature was used, it's now `Feature`.
```

### Mapped-type spec format

CLI: `--mapped-type schema:import:type[:alias]`

| Position | Field    | Meaning                                                       |
|----------|----------|---------------------------------------------------------------|
| 1        | `schema` | Schema name from `components.schemas` to replace              |
| 2        | `import` | Module specifier to import from                               |
| 3        | `type`   | Named export to import                                        |
| 4        | `alias`  | Optional. Rename to avoid local collisions (`Feature as Geo`) |

The same shape works in JS/TS/YAML/JSON configs:

```ts
mappedTypes: [
  { schema: 'GeoFeature',           import: 'geojson', type: 'Feature' },
  { schema: 'GeoFeatureCollection', import: 'geojson', type: 'FeatureCollection' },
  { schema: 'DateTime',             import: 'dayjs',   type: 'Dayjs' },
  { schema: 'BigDecimal',           import: 'decimal.js', type: 'Decimal', alias: 'BigDecimal' },
]
```

## Response type mapping

By default the generator picks the request flavor from the operation's
declared response content type: `application/json` → JSON,
`application/octet-stream` → Blob, `text/*` → text, and so on. Override
the picker per content type with `responseTypeMapping` when the spec
under-declares the wire format (e.g. a route returns
`application/pdf` but is declared as JSON):

```ts
responseTypeMapping: [
  { contentType: 'application/pdf', responseType: 'blob' },
  { contentType: 'application/x-ndjson', responseType: 'text' },
]
```

| Field          | Meaning                                                                                                  |
|----------------|----------------------------------------------------------------------------------------------------------|
| `contentType`  | Response media type to override (matched case-insensitively against the operation's declared responses). |
| `responseType` | One of `json`, `blob`, `text`, `arrayBuffer` — mirrors Angular's `HttpClient.request({ responseType })`. |

The matched operation is wired through the corresponding
`requestFactory.*` variant (see [Non-JSON responses](/guides/angular/#non-json-responses)).
This option is **not** exposed on the CLI; configure it via a config
file or the Node API.

## Customizing names

By default, openapi-ng derives names like this:

| Key          | Default chain                                                                  |
|--------------|--------------------------------------------------------------------------------|
| `methodName` | `camelCase(operationId)`, else `camelCase(method + '_' + pathSegments.join('_'))` |
| `group`      | `pascalCase(tags[0])`, else `pascalCase(pathSegments[0])`, else `'Default'`    |

These run unless you set the corresponding `naming.methodName` /
`naming.group` config. **Both keys support fallback chains** — a chain
runs each rule in order until one succeeds.

### Rule shape

```ts
{
  from?: string,        // template expanded to the parser input
  parse?: RegExp,       // optional; named captures populate {capture.*}
  format?: string,      // template expanded to the final name
  case?: 'camel' | 'pascal' | 'snake' | 'kebab' | 'constant',
}
```

A rule fails when:

- Any `{token}` in `from` or `format` is unbound (e.g. `{operationId}`
  on an operation with no `operationId`, or `{x-foo}` when the
  extension isn't present).
- `parse` is set and the regex doesn't match the expanded `from`.
- Either template is malformed (unclosed `{`, bad index syntax).

If every rule in a chain fails, the generator throws
`E_POLICY_VIOLATION` with `subcode: 'naming-resolution'`.

### Template tokens

Both `from` and `format` accept the same tokens:

| Token              | Resolves to                                                                                     |
|--------------------|-------------------------------------------------------------------------------------------------|
| `{operationId}`    | The OpenAPI `operationId`. Unbound when missing.                                                |
| `{method}`         | HTTP method, lowercased (`get`, `post`, …).                                                     |
| `{path}`           | The full path (e.g. `/users/{id}`).                                                             |
| `{pathSegments[N]}`| Nth path segment (0-indexed). Brace path params are unwrapped — `/users/{id}` → `users`, `id`. |
| `{tags[N]}`        | Nth tag.                                                                                        |
| `{x-<name>}`       | Vendor extension on the operation. *Not yet plumbed through normalize — always unbound today.* |
| `{capture.<name>}` | Named capture from `parse` (`(?<name>...)`). Only available inside `format`.                    |

Array tokens accept **negative indexes** that count from the tail:
`{tags[-1]}` is the last tag, `{pathSegments[-1]}` the deepest segment.

### `case` values

Five options. The transformer first splits the input into tokens
(splitting on case boundaries, separators, and digit/letter
transitions), then rejoins:

| Case       | Input `get_someThing`  |
|------------|------------------------|
| `camel`    | `getSomeThing`         |
| `pascal`   | `GetSomeThing`         |
| `snake`    | `get_some_thing`       |
| `kebab`    | `get-some-thing`       |
| `constant` | `GET_SOME_THING`       |

Values are lowercase per spec — `'Camel'` does not parse.

### Forms

A `NamingValue` can be one of three shapes:

```ts
// 1. Shorthand: a bare format template. No case transform.
methodName: '{operationId}'

// 2. Single rule.
methodName: {
  from: '{operationId}',
  parse: /^[^_]+_(?<rest>.+)$/,
  format: '{capture.rest}',
  case: 'camel',
}

// 3. Chain: try each entry in order, stop on first success.
methodName: [
  // Prefer the vendor extension if the spec sets one.
  { from: '{x-method-name}', format: '{x-method-name}', case: 'camel' },
  // Strip a verb prefix from operationId: `posts_listAll` → `listAll`.
  {
    from: '{operationId}',
    parse: /^[^_]+_(?<rest>.+)$/,
    format: '{capture.rest}',
    case: 'camel',
  },
  // Plain operationId.
  '{operationId}',
]
```

### Examples

**Strip a prefix from `operationId`**

```ts
naming: {
  methodName: {
    from: '{operationId}',
    parse: /^[^_]+_(?<rest>.+)$/,
    format: '{capture.rest}',
    case: 'camel',
  },
}
// posts_listAll → listAll
```

**Group by a vendor extension, fall back to the first tag**

```ts
naming: {
  group: [
    { format: '{x-resource-group}', case: 'pascal' },
    { format: '{tags[0]}',          case: 'pascal' },
  ],
}
```

**Build a method name from method + last path segment**

```ts
naming: {
  methodName: {
    from: '{method}_{pathSegments[-1]}',
    format: '{method}_{pathSegments[-1]}',
    case: 'camel',
  },
}
// GET /users/{id} → getId   ⚠ may collide; chain a fallback in real use
```

**Force kebab-case file groups**

```ts
naming: {
  group: { format: '{tags[0]}', case: 'kebab' },
}
// tag "PetOrders" → pet-orders
```

### YAML/JSON limitation

YAML and JSON configs accept the same shape, with one limitation: the
`parse` field requires a JavaScript `RegExp` and cannot be expressed
in YAML/JSON. Use an `openapi-ng.config.{ts,js,mjs,cjs}` config file
when you need `parse` rules.
