---
title: Node API
description: Programmatic surface — generate(), GenerateOptions, GenerateResult, and the Config file-config shape.
---

`openapi-ng` is a CLI-first tool, but every flag has a matching
programmatic option. Use the Node API when you want to embed generation
into a build script, a custom tool, or a test fixture.

## `generate`

```ts
import { generate } from '@avsystem/openapi-ng';

const result = await generate({
  inputPath: './spec.yaml',
  outputPath: './generated',
  emit: ['models', 'angular'],
});
```

Returns a [`GenerateResult`](#generateresult). Throws a
[`GenerateError`](/reference/diagnostics/#generateerror) on fatal
diagnostics; pre-fatal warnings ride on `error.warnings`.

Omit `outputPath` to keep the result fully in memory — every artifact
still shows up in `result.artifacts`.

## `GenerateOptions`

```ts
interface GenerateOptions {
  inputPath?: string;
  inputContents?: string;
  displayPath?: string;
  inputFormat?: InputFormat;          // const enum: 'json' | 'yaml'
  outputPath?: string;
  emit?: Array<EmitTarget>;           // const enum: 'models' | 'angular'
  mappedTypes?: Array<MappedType>;
  responseTypeMapping?: Array<ResponseTypeMapping>;
  naming?: NamingConfig;
}
```

`InputFormat`, `EmitTarget`, and `ResponseType` (used below) are
exported both as TypeScript types and as runtime `const` objects, so
you can use them either as string literals or as named constants:

```ts
import { EmitTarget, InputFormat, generate } from '@avsystem/openapi-ng';

await generate({
  inputPath: './spec.yaml',
  inputFormat: InputFormat.Yaml,            // or 'yaml'
  emit: [EmitTarget.Models, EmitTarget.Angular],  // or ['models', 'angular']
});
```

| Field                   | Notes                                                                                                                                                                                                            |
|-------------------------|------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `inputPath`             | Path to the spec on disk. Mutually exclusive with `inputContents`; setting both or neither is rejected.                                                                                                          |
| `inputContents`         | Raw spec source. When set, `displayPath` is required and `OPENAPI_NG_MAX_INPUT_BYTES` applies to the byte length.                                                                                                |
| `displayPath`           | Banner / diagnostic display string. Required with `inputContents`; ignored with `inputPath` (the path itself is normalised).                                                                                     |
| `inputFormat`           | Decoder hint, only honoured with `inputContents`. Combining with `inputPath` is a shape error.                                                                                                                   |
| `outputPath`            | Omit for in-memory mode. Empty string is rejected.                                                                                                                                                               |
| `emit`                  | Defaults to `['models', 'angular']`. Selecting `'angular'` alone auto-includes `'models'` with a warning.                                                                                                        |
| `mappedTypes`           | Per-schema overrides — point a generated schema at an external import. See the [Configuration guide](/guides/configuration/).                                                                                    |
| `responseTypeMapping`   | Per-content-type override for the response decoding kind (`json` / `blob` / `text` / `arrayBuffer`). Matched case-insensitively against the spec's media types. Wires the right `requestFactory.*` variant.     |
| `naming`                | Customise emitted method and group names. See the [Configuration guide](/guides/configuration/).                                                                                                                 |

### `MappedType`

```ts
interface MappedType {
  schema: string;   // schema name as it appears in #/components/schemas
  import: string;   // package or relative path to import from
  type: string;     // exported type name in that module
  alias?: string;   // local alias if it would otherwise collide
}
```

### `ResponseTypeMapping`

```ts
interface ResponseTypeMapping {
  contentType: string;                                   // matched case-insensitively
  responseType: ResponseType;                            // const enum: 'json' | 'blob' | 'text' | 'arrayBuffer'
}
```

Names mirror Angular's `HttpClient.request({ responseType })` and
`httpResource.<kind>()` factories so the config vocabulary stays in JS
conventions.

### `NamingConfig`

```ts
interface NamingConfig {
  methodName?: Naming;
  group?: Naming;
}

type Naming = string | NamingRule | Array<NamingRule | string>;

interface NamingRule {
  from?: string;
  parse?: RegExp;
  format?: string;
  case?: Case;     // 'camel' | 'pascal' | 'snake' | 'kebab' | 'constant'
}
```

See the [Configuration guide](/guides/configuration/) for evaluation
order, fallback chains, and worked examples. `parse` requires a real
`RegExp`, so YAML/JSON configs cannot express rules that match input —
use an `openapi-ng.config.ts` / `.js` / `.mjs` / `.cjs` file in that
case.

## `GenerateResult`

```ts
interface GenerateResult {
  summary: GenerateSummary;
  diagnostics: Array<GeneratorDiagnostic>;
  artifacts: Array<GeneratedArtifact>;
}

interface GenerateSummary {
  normalizedSourcePath: string;
  specVersion: string;
  title: string;
  pathCount: number;
  operationCount: number;
  schemaCount: number;
}

interface GeneratedArtifact {
  path: string;       // relative output path
  contents: string;   // emitted source, always present
}
```

`diagnostics` carries warnings on success. On fatal failure the
function throws a `GenerateError`; any pre-fatal warnings ride on
`error.warnings`. See [Diagnostics](/reference/diagnostics/) for the
code/subcode taxonomy and the `GenerateError` class shape.

## `Config` — file-config shape

The shape accepted by `.openapi-ng.{yaml,json}`,
`openapi-ng.config.{ts,mts,cts,mjs,js,cjs}`, and the `defineConfig`
helper. Keys mirror the YAML/JSON surface (`input`/`output`), **not**
the programmatic `generate({ inputPath, outputPath, ... })` surface.

```ts
interface Config {
  input?: string;
  output?: string;
  emit?: Array<EmitTarget>;
  mappedTypes?: Array<MappedType>;
  responseTypeMapping?: Array<ResponseTypeMapping>;
  naming?: NamingConfig;
}
```

```ts
import { defineConfig } from '@avsystem/openapi-ng/config';

export default defineConfig({
  input: 'petstore.openapi.yaml',
  output: './generated',
  responseTypeMapping: [
    { contentType: 'application/pdf', responseType: 'blob' },
  ],
});
```

`defineConfig` is an identity helper — it returns its argument
unchanged and exists purely to anchor TypeScript inference.

## Error handling

```ts
import { GenerateError, generate } from '@avsystem/openapi-ng';

try {
  await generate({ inputPath: 'spec.yaml', emit: ['models', 'angular'] });
} catch (err) {
  if (err instanceof GenerateError) {
    console.error(`[${err.code}] ${err.message}`);
    if (err.subcode) console.error(`  subcode: ${err.subcode}`);
    if (err.warnings.length) console.warn(err.warnings);
  }
}
```

`GenerateError.isGenerateError(value)` is the cross-realm-safe
predicate — use it when the error may travel across worker threads, vm
contexts, or iframes where `instanceof` matches on class identity
rather than the embedded sentinel.

See [Diagnostics](/reference/diagnostics/) for the full code/subcode
table and remediation guide.
