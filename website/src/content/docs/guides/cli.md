---
title: CLI
description: Reference for the openapi-ng command-line interface — generate and init commands, their flags, and URL inputs.
---

The `openapi-ng` binary exposes two top-level commands: `generate` and
`init`.

## `openapi-ng generate`

```
openapi-ng generate --input <path> [options]
```

| Flag                   | Short | Description                                                                                                                                                            |
|------------------------|-------|------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `--input <path>`       | `-i`  | Path to an OpenAPI 3.x JSON or YAML file, or an `https://` URL (required). See [URL inputs](#url-inputs) below.                                                        |
| `--output <dir>`       | `-o`  | Output directory. Omit to generate in memory without writing files                                                                                                     |
| `--emit <targets>`     |       | Comma-separated emit list: `models,angular` (repeatable). Default: `models,angular`. `'angular'` auto-includes `'models'`.                                             |
| `--verbose`            |       | Print warning diagnostics with codes                                                                                                                                   |
| `--mapped-type <spec>` |       | Replace a schema with an external type import (repeatable). Format: `schema:import:type[:alias]` — see [Mapped types](/guides/configuration/#mapped-type-spec-format). |
| `--config <path>`      | `-c`  | Path to a config file (overrides auto-discovery — see [Configuration](/guides/configuration/))                                                                         |

### URL inputs

`--input` (and the `input` config key, and `generate({ inputPath })`)
accept an `https://` URL alongside local file paths:

```bash
openapi-ng generate -i https://example.com/openapi.yaml -o ./src/generated
```

Only `https://` is supported — `http://` and other schemes are
rejected. Redirects are followed up to 5 hops and must stay on
`https://`. The fetched body counts against the same input cap that
applies to local files (`OPENAPI_NG_MAX_INPUT_BYTES`, default 16 MiB).
The fetch wall-clock timeout defaults to 30 s and is configurable via
`OPENAPI_NG_INPUT_TIMEOUT_MS`. See [Environment variables](/reference/environment/)
for both knobs.

## `openapi-ng init`

```
openapi-ng init [--format yaml|json|ts|js]
```

Writes a starter config file in the current directory. Default format
is `yaml`. `--format ts` writes `openapi-ng.config.mts` (with a
`defineConfig` import and a commented `naming.parse` RegExp example);
`--format js` writes `openapi-ng.config.mjs`. Both pick the `.m*`
extension on purpose: it forces ESM, avoiding Node's CJS-first parse
+ ESM-reparse penalty under a typeless `package.json`, and keeps the
config independent of `package.json#type` mutations.

Aborts (no overwrite) if any of the eight discoverable config files
already exists in the current directory.
