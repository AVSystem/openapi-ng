# openapi-ng

Generate TypeScript models and Angular services from OpenAPI 3.x specs — fast, deterministic, Rust-powered.

**[Documentation →](https://docs.openapi-ng.dev)** · [Getting started](https://docs.openapi-ng.dev/getting-started/) · [Angular guide](https://docs.openapi-ng.dev/guides/angular/) · [Node API](https://docs.openapi-ng.dev/reference/node-api/) · [Diagnostics](https://docs.openapi-ng.dev/reference/diagnostics/)

## Why openapi-ng

- **Rust-powered codegen.** The engine is a native binary loaded via [NAPI-RS](https://napi.rs). The same input always produces identical output.
- **Angular-first output.** Each operation ships with three flavors — `.observable()`, `.resource()`, `.request()` — matching Angular's current HTTP primitives.
- **Strict OpenAPI subset.** A focused 3.x slice with clear diagnostics. No silent misgeneration; see [Assumptions & limitations](https://docs.openapi-ng.dev/reference/limitations/) for the accepted shape.
- **Configurable naming.** Tune method names and service grouping with template + regex rules, via YAML, JSON, or TypeScript config.

## Install

```bash
pnpm add -D @avsystem/openapi-ng
```

Requires Node.js >= 18. Pre-built binaries for macOS, Linux, and Windows (x64 / ARM64); a WASI fallback covers other targets. See [Runtime & platforms](https://docs.openapi-ng.dev/reference/runtime/).

## Quickstart

```bash
openapi-ng generate --input petstore.openapi.yaml --output ./generated
```

```
✓ Generated 4 files from Petstore (3.0.3)
  1 path · 1 operation · 1 schema

  model.generated.ts
  rest.model.ts
  rest.util.ts
  rest/pet.rest.generated.ts
```

Wire a generated service into a component:

```ts
import { Component, inject } from '@angular/core';
import { PetRest } from './generated/rest/pet.rest.generated';

@Component({ /* ... */ })
export class PetList {
  readonly #pets = inject(PetRest);

  // Signal-based, reactive, with a default value while loading.
  readonly list = this.#pets.listPets.resource({ defaultValue: [] });
}
```

Full walkthrough on [docs.openapi-ng.dev/getting-started](https://docs.openapi-ng.dev/getting-started/).

## Development

```bash
pnpm install
pnpm build         # release build (Rust + NAPI)
pnpm build:debug   # debug build (faster compile)
pnpm test          # Node integration tests (AVA)
cargo test         # Rust unit tests
pnpm lint          # oxlint
pnpm format        # oxfmt + rustfmt + taplo
```

Rust changes require a rebuild (`pnpm build` or `pnpm build:debug`) before tests reflect them.

Bug reports and PRs welcome at [github.com/AVSystem/openapi-ng](https://github.com/AVSystem/openapi-ng).

## License

[MIT](./LICENSE) © 2026 pkurcx
