---
title: Diagnostics
description: Diagnostic codes, subcodes, and the GenerateError class — what fires, when, and what to check.
---

## Diagnostic codes

```ts
type DiagnosticCode =
  | 'E_INPUT_INVALID'
  | 'E_INVALID_OPTION'
  | 'E_INVALID_REFERENCE'
  | 'E_POLICY_VIOLATION'
  | 'E_UNEXPECTED'
  | 'E_UNSUPPORTED_RUNTIME'
  | 'E_UNSUPPORTED_SEMANTIC'
  | 'E_WRITE_FAILED';
```

| Code                     | When                                                                                                                                                               | What to check                                                                                                                                                                                                                                     |
|--------------------------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `E_INPUT_INVALID`        | The input file couldn't be read, decoded as JSON/YAML, or exceeds `OPENAPI_NG_MAX_INPUT_BYTES`.                                                                    | File path, file extension (`.json`/`.yaml`/`.yml`), JSON/YAML syntax (the message includes line:column when the parser supplies it), and the input-bytes cap — see [Environment variables](/reference/environment/).                              |
| `E_INVALID_OPTION`       | Caller passed an invalid option. Subcode `'shape'` is a wrapper-side type/shape rejection; no subcode means the Rust core rejected an option's value semantically. | Compare your `generate()` call against `index.d.ts`; branch on `err.subcode === 'shape'` when the distinction matters.                                                                                                                            |
| `E_INVALID_REFERENCE`    | A `$ref` doesn't resolve or points outside `#/components/schemas/...`.                                                                                             | Typos in `$ref`, external-file refs (not supported), refs to other components sections, cycles in non-schema positions.                                                                                                                           |
| `E_POLICY_VIOLATION`     | An IR-level rule is broken.                                                                                                                                        | Inspect `err.subcode` to route. Cap-exceeded subcodes correlate with the env-var knobs (see [Environment variables](/reference/environment/)); the `multipart-*` / `urlencoded-*` subcodes pin the precise reject path for the form-body walkers. |
| `E_UNEXPECTED`           | A Rust panic crossed the NAPI boundary. The library swallows the panic and surfaces it as an error so your Node process keeps running.                             | Open an issue with the message — this should never fire on valid input.                                                                                                                                                                           |
| `E_UNSUPPORTED_RUNTIME`  | The package was loaded in a non-Node runtime (browser bundle, Cloudflare Workers, Vercel Edge, etc.) and `generate()` was called on the `browser.js` stub.         | Run the generator from Node only; remove `@avsystem/openapi-ng` from browser bundles or alias it away. See [Runtime & platforms](/reference/runtime/).                                                                                            |
| `E_UNSUPPORTED_SEMANTIC` | The spec uses an OpenAPI shape outside the supported subset.                                                                                                       | See the [Angular generator](/guides/angular/) limitations. Warn-level by default; if blocking, switch to a supported shape.                                                                                                                       |
| `E_WRITE_FAILED`         | An output file write failed.                                                                                                                                       | Permissions on `outputPath`, disk space, parent-directory existence.                                                                                                                                                                              |

## Subcodes

```ts
type DiagnosticSubcode =
  | 'shape'
  | 'duplicate-operation-id'
  | 'duplicate-schema-name'
  | 'field-collision'
  | 'format-dropped'
  | 'mapping-expansion-exceeded'
  | 'missing-discriminator-property'
  | 'missing-operation-id'
  | 'missing-tag'
  | 'multi-content-body'
  | 'multipart-composed-field'
  | 'multipart-nested-object'
  | 'multipart-non-object-body'
  | 'multipart-open-schema'
  | 'naming-resolution'
  | 'operation-cap-exceeded'
  | 'schema-cap-exceeded'
  | 'unsupported-body-content-type'
  | 'unsupported-parameter-location'
  | 'urlencoded-binary-field'
  | 'urlencoded-composed-field'
  | 'urlencoded-nested-object'
  | 'urlencoded-non-object-body'
  | 'urlencoded-open-schema';
```

Subcodes are populated for routable diagnostics
(`E_POLICY_VIOLATION`, select `E_UNSUPPORTED_SEMANTIC` warnings, and
`E_INVALID_OPTION` wrapper-side shape rejections); they are `null`
otherwise.

`E_INVALID_OPTION` is dual-sourced: the JS wrapper raises it with
`subcode: 'shape'` for pre-flight type/shape failures (e.g. `emit must
be an array of EmitTarget`); the Rust core raises it with no subcode
for semantic rejections of an option's value. Branch on
`err.subcode === 'shape'` if the distinction matters.

## `GeneratorDiagnostic`

```ts
interface GeneratorDiagnostic {
  code: DiagnosticCode;
  subcode: DiagnosticSubcode | null;
  severity: 'error' | 'warning';
  message: string;
  path: string;
}
```

`generate()` returns warnings in `result.diagnostics` on success. On
fatal failure it throws a `GenerateError` whose `warnings` field
contains any pre-fatal warnings.

## `GenerateError`

```ts
class GenerateError extends Error {
  readonly code: DiagnosticCode;                  // e.g. 'E_INPUT_INVALID'
  readonly subcode: DiagnosticSubcode | null;     // finer-grained routing key
  readonly path: string;                          // input path (empty when not applicable)
  readonly warnings: Array<GeneratorDiagnostic>;  // pre-fatal warnings (empty when none)
}
```

```ts
import { GenerateError, generate } from '@avsystem/openapi-ng';

try {
  generate({ inputPath: 'spec.yaml', emit: ['models', 'angular'] });
} catch (err) {
  if (err instanceof GenerateError) {
    console.error(`[${err.code}] ${err.message}`);
    if (err.subcode) console.error(`  subcode: ${err.subcode}`);
    if (err.warnings.length) console.warn(err.warnings);
  }
}
```
