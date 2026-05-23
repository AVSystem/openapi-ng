---
title: Getting started
description: Install openapi-ng and generate your first TypeScript models and Angular services from an OpenAPI spec.
---

## Install

```bash
pnpm add -D @avsystem/openapi-ng
```

`openapi-ng` requires Node.js 18+. See [Runtime & platforms](/reference/runtime/)
for the full compatibility matrix.

## Generate from the CLI

```bash
openapi-ng generate --input petstore.openapi.yaml --output ./generated
```

Output:

```
✓ Generated 4 files from Petstore (3.0.3)
  1 path · 1 operation · 1 schema

  model.generated.ts
  rest.model.ts
  rest.util.ts
  rest/pet.rest.generated.ts
```

The full flag list is on the [CLI page](/guides/cli/).

## Generate from Node

```js
import { generate } from '@avsystem/openapi-ng';

const result = await generate({
  inputPath: './petstore.openapi.yaml',
  outputPath: './generated',
  emit: ['models', 'angular'],
});

console.log(result.summary);
console.log(result.diagnostics);
console.log(result.artifacts);
```

Omit `outputPath` to keep the result entirely in memory.

## What you get

| Artifact          | File                            | Description                                          |
|-------------------|---------------------------------|------------------------------------------------------|
| TypeScript models | `model.generated.ts`            | Interfaces, type aliases, and string enum unions     |
| Angular support   | `rest.model.ts`, `rest.util.ts` | HTTP helper types and request utilities              |
| Angular services  | `rest/{tag}.rest.generated.ts`  | `@Injectable` service classes grouped by OpenAPI tag |

See [Angular generator](/guides/angular/) for the shape of the emitted
services and example component usage.

## Next steps

- Tune naming and mapped types on the [Configuration](/guides/configuration/) page.
- Walk through the [Angular generator](/guides/angular/) to see what the services look like and how to consume them.
- Skim the [Diagnostics reference](/reference/diagnostics/) so error codes don't surprise you later.
