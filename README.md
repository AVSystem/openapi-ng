# openapi-ng

Generate TypeScript models and Angular services from OpenAPI 3.x specs — fast, deterministic, Rust-powered.

**[Documentation →](https://docs.openapi-ng.dev)** · [Getting started](https://docs.openapi-ng.dev/getting-started/) · [Angular guide](https://docs.openapi-ng.dev/guides/angular/) · [Node API](https://docs.openapi-ng.dev/reference/node-api/) · [Diagnostics](https://docs.openapi-ng.dev/reference/diagnostics/)

## Why openapi-ng

- **Rust-powered codegen.** The engine is a native binary loaded via [NAPI-RS](https://napi.rs). The same input always produces identical output.
- **Angular-first output.** Each operation ships with three flavors — `.observable()`, `.resource()`, `.request()` — matching Angular's current HTTP primitives.
- **Strict OpenAPI subset.** A focused 3.x slice with clear diagnostics. No silent misgeneration; see [Assumptions & limitations](https://docs.openapi-ng.dev/reference/limitations/) for the accepted shape.
- **Configurable naming.** Tune method names and service grouping with template + regex rules, via YAML, JSON, or TypeScript config.
- **Thin, pass-through helpers.** Generated methods just build the request (method, URL, query, body) and forward every `HttpClient.request` / `httpResource` option through unchanged — `withCredentials`, `transferCache`, `reportProgress`, `equal`, `injector`, and the rest. The response reaches you untouched.

## Install

```bash
bun add -d @avsystem/openapi-ng
```

Requires Node.js >= 18. Pre-built binaries for macOS, Linux, and Windows (x64 / ARM64). See [Runtime & platforms](https://docs.openapi-ng.dev/reference/runtime/).

## Quickstart

```bash
openapi-ng generate --input petstore.openapi.yaml --output ./generated
```

```
✓ Generated 5 files from Petstore (3.0.3)
  1 path · 1 operation · 1 schema

  model.generated.ts
  rest.model.ts
  rest.util.ts
  rest.validate.ts
  rest/pet.rest.generated.ts
```

Wire a generated service into a component:

```ts
import { Component, inject } from '@angular/core';
import { PetRest } from './generated/rest/pet.rest.generated';

@Component({
  /* ... */
})
export class PetList {
  readonly #pets = inject(PetRest);

  // Signal-based, reactive, with a default value while loading.
  readonly list = this.#pets.listPets.resource({ defaultValue: [] });
}
```

### Signal-forms async validation: `rest.validate.ts`

A `validateRest(path, restMethod, opts)` helper wraps Angular signal-forms `validateAsync` and delegates to the generated `RequestFn.resource()`, preserving request/response typing:

```ts
import { validateRest } from './generated/rest.validate';

validateRest(emailPath, accountRest.checkEmail, {
  request: (ctx) => ({ email: ctx.value() }),
  onError: () => ({ kind: 'email-taken' }),
});
```

`@angular/forms` is an optional peer — install it only if you import from `rest.validate.ts`; the file tree-shakes away when unused.

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

[MIT](./LICENSE) © 2026
