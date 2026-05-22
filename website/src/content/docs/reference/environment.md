---
title: Environment variables
description: Generator caps that protect against pathological inputs — input size, fetch timeout, schema/operation counts, YAML expansion ratio, and the SSRF guard escape hatch.
---

Five optional caps guard the generator against pathological inputs
(e.g. an accidentally fanned-out YAML anchor or a multi-megabyte spec)
before any heavy work runs. Defaults are deliberately generous — real
specs sit several orders of magnitude below — and all five accept a
positive integer override. A sixth variable opts out of the URL-fetch
SSRF guard for local/test workflows.

| Variable                         | Default             | Effect on overflow                                          |
|----------------------------------|---------------------|-------------------------------------------------------------|
| `OPENAPI_NG_MAX_INPUT_BYTES`     | `16777216` (16 MiB) | `E_INPUT_INVALID` before parsing                            |
| `OPENAPI_NG_INPUT_TIMEOUT_MS`    | `30000` (30 s)      | URL fetch aborts                                            |
| `OPENAPI_NG_MAX_SCHEMAS`         | `10000`             | `E_POLICY_VIOLATION` / `schema-cap-exceeded`                |
| `OPENAPI_NG_MAX_OPERATIONS`      | `10000`             | `E_POLICY_VIOLATION` / `operation-cap-exceeded`             |
| `OPENAPI_NG_MAX_EXPANSION_RATIO` | `50`                | `E_POLICY_VIOLATION` / `mapping-expansion-exceeded`         |
| `OPENAPI_NG_ALLOW_PRIVATE_HOSTS` | *unset*             | When set to `1`, disables the SSRF guard on `--input <url>` |

## Per-variable detail

### `OPENAPI_NG_MAX_INPUT_BYTES`

Maximum input size in bytes (file, URL, or inline `inputContents`).
Inputs that exceed the cap fail with `E_INPUT_INVALID` before parsing.

### `OPENAPI_NG_INPUT_TIMEOUT_MS`

Wall-clock cap on URL fetches. Applies to the total time including
redirects.

### `OPENAPI_NG_MAX_SCHEMAS`

Maximum number of entries allowed under `components.schemas`.
Exceeding the cap fails with `E_POLICY_VIOLATION` and subcode
`schema-cap-exceeded`.

### `OPENAPI_NG_MAX_OPERATIONS`

Maximum total number of operations summed across all paths. Exceeding
the cap fails with `E_POLICY_VIOLATION` and subcode
`operation-cap-exceeded`.

### `OPENAPI_NG_MAX_EXPANSION_RATIO`

Maximum acceptable ratio between the re-serialised parsed YAML and the
source bytes. YAML anchors that fan out aggressively (one alias
inlined into hundreds of mapping entries) produce a re-serialised tree
orders of magnitude larger than the source; the byte-size cap above
cannot catch this because the source itself stays small. Exceeding the
ratio fails with `E_POLICY_VIOLATION` and subcode
`mapping-expansion-exceeded`.

### `OPENAPI_NG_ALLOW_PRIVATE_HOSTS`

URL inputs (`--input https://…`) are blocked by an SSRF guard from
resolving to private/loopback/link-local addresses by default. Set
`OPENAPI_NG_ALLOW_PRIVATE_HOSTS=1` to disable the guard for the
current process — useful when pointing the CLI at a spec served from
`localhost` or an internal mirror during development.

## Lifecycle

Each variable is read once per process; invalid or empty values fall
back to the default silently.
