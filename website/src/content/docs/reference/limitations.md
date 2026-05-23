---
title: Assumptions & limitations
description: The OpenAPI 3.x subset openapi-ng accepts — required fields, supported schema shapes, response/body rules, and what's out of scope.
---

`openapi-ng` targets a focused subset of OpenAPI 3.x. Specs outside
this subset are rejected with clear diagnostics — no silent
misgeneration. Most rejections carry a `subcode` that pins the precise
shape your spec violated; see the
[Diagnostics reference](/reference/diagnostics/) for routing.

## Spec requirements

- **`operationId` is required** on every operation. The generator uses
  it to name service methods and request interfaces.
- **Every operation needs at least one `tag`**. The first tag
  determines which service class the operation belongs to (tag `pet` →
  `PetRest`). Additional tags are ignored for grouping.
- **Only `#/components/schemas/` references** are supported. External
  files, URL refs, and refs to other sections (e.g.
  `#/components/responses/`) are rejected.

## Response handling

- **Only the lowest 2xx response is used.** If your operation defines
  both `200` and `201`, the `200` response is picked. Error responses
  (4xx, 5xx) are completely ignored — no error type mappings are
  generated.
- Default content type is `application/json`; other types route to the
  matching `requestFactory.{blob,text,arrayBuffer}` variant (see the
  [Angular generator](/guides/angular/#non-json-responses)).

## Request bodies

- **`application/json`, `multipart/form-data`, and
  `application/x-www-form-urlencoded`** are supported. Other content
  types — XML, custom media types, and bodies declaring multiple media
  types on a single request — are rejected.

## Parameters

- **Path and query parameters** are fully supported and rendered at the
  top of the request interface.
- **Header parameters** are accepted and emitted as a sibling `headers`
  object on the request interface (alongside path/query and the body
  surface), so callers pass them in the same call.
- **Cookie parameters** are accepted but **omitted from the generated
  service contract** with an `E_UNSUPPORTED_SEMANTIC` /
  `unsupported-parameter-location` warning — cookies are managed by the
  browser, so surfacing them on the client API would be misleading.

## Request body shape

The generated `*Params` interface treats the body using a
smart-flatten rule keyed on how the spec author wrote the schema:

- A body declared as `$ref: '#/components/schemas/Name'` (or any
  non-object JSON shape — scalar, array, union) surfaces as a single
  nested `body: Name` field, preserving the named type.
- A body declared inline as `type: object` has its properties hoisted
  to top-level fields next to path/query.
- Multipart and url-encoded bodies always hoist their fields to
  top-level (binary fields surface as `Blob | File`).

The escape hatch is in the spec: name the schema to nest under `body`,
or inline a schema's contents to hoist its properties. Hoisted body
properties that duplicate a path or query parameter name are rejected
at codegen with `E_POLICY_VIOLATION` / `field-collision`.

## Schemas

- **String enums only.** Integer or mixed enums are rejected.
- **`not` keyword** is not supported.
- **`additionalProperties`** works only on pure object schemas — emitted
  as `Record<string, T>`. Cannot be combined with named `properties`,
  `$ref`, or composition keywords (`allOf` / `oneOf` / `anyOf`). Boolean
  `additionalProperties: true` is also rejected; it must be a schema.
- **One composition keyword per schema.** You can use `allOf`, `oneOf`,
  or `anyOf`, but not two of them on the same schema.
- **`oneOf` with `discriminator: { propertyName }`** emits `A | B` on
  the TypeScript surface, with each variant's discriminator property
  narrowed to its wire value. When `discriminator.mapping` is set, the
  mapping key becomes the literal value (e.g. `mapping: { feline:
  '#/.../Cat' }` produces `kind: 'feline'` on the `Cat` variant). When
  no mapping entry matches, the lowercased schema name is used as a
  fallback.
- **Recursive schemas** are supported.
- **Nullable** types via OpenAPI 3.0's `nullable: true` are supported,
  emitted as `T | null`.

## Out of scope

- Remote URLs or external file references (the `--input <url>` fetcher
  is the one exception).
- OpenAPI 2.x (Swagger) and 3.1-specific features.
- Error response type generation.
- Custom service grouping strategies (tag-first only).
