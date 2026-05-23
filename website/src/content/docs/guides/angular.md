---
title: Angular generator
description: What the Angular generator emits — services, the three operation flavors (observable, resource, request), typed parse, base-path wiring, and non-JSON response variants.
---

## Models

Schemas become typed TypeScript — interfaces for objects, type aliases
for references, and multi-line string unions for enums:

```ts
export interface Pet {
  id: PetId;
  name: string;
  nickname?: string;
  status: PetStatus;
  tags: Tag[];
}

export type PetId = string;

export type PetList = Pet[];

export type PetStatus =
  | 'available'
  | 'pending'
  | 'sold';
```

Properties are sorted alphabetically. Optional fields get `?`. All
output is deterministic.

## Services

Each OpenAPI tag becomes an `@Injectable` Angular service. Every
operation is a readonly property with three flavors:

- **`.observable(req, options?)`** — returns an `Observable<T>` via `HttpClient`. Pass `{ observe: 'response' }` to receive `Observable<HttpResponse<T>>` (headers, status), or `{ observe: 'events' }` for `Observable<HttpEvent<T>>` (upload/download progress, raw events). See [`.observable()` modes](#observable-modes) below.
- **`.resource(reactiveReq, options?)`** — returns an `HttpResourceRef<T>` via Angular's `httpResource`. The ref exposes `headers()`, `statusCode()`, and `progress()` as signals.
- **`.request(req)`** — returns the raw `CommonRequest` for custom use

```ts
@Injectable({
  providedIn: 'root',
})
export class PetRest {
  readonly listPets = requestFactory.zeroArg<PetList>(
    () => ({
      method: 'GET',
      url: `/pets`,
    }),
  );

  readonly getPet = requestFactory<GetPetParams, Pet>(
    (request: GetPetParams) => {
      const { petId } = request;
      return {
        method: 'GET',
        url: `/pets/${encodeURIComponent(petId)}`,
      };
    },
  );

  readonly updatePet = requestFactory<UpdatePetParams, Pet>(
    (request: UpdatePetParams) => {
      const { petId, includeHistory, body } = request;
      return {
        method: 'POST',
        url: `/pets/${encodeURIComponent(petId)}`,
        params: httpParams({ includeHistory }),
        body: body,
      };
    },
  );
}
```

Operations with no request parameters use `requestFactory.zeroArg<T>` (or
`.zeroArg.blob` / `.zeroArg.text` / `.zeroArg.arrayBuffer` for non-JSON
responses); parameterised operations use the top-level
`requestFactory<Req, Res>` (with the same `.blob` / `.text` /
`.arrayBuffer` siblings). `HttpClient` is injected internally by the
runtime — services do not declare it.

### Request interface shape

Path params and query params always sit at the top of the request
interface. The body's surface follows a smart-flatten rule keyed on
how the spec author wrote the schema:

- **Named `$ref` body** — preserved as a single nested `body: RefName`
  field so the spec's named type stays referenceable from your code
  (`UpdatePetParams['body']` is the imported `UpdatePetRequest`).
- **Inline `type: object` body** — hoisted to top-level fields next to
  path/query, matching the spec's authorial signal of "these are just
  parameters, not a named DTO".
- **Form bodies** (`multipart/form-data`, `application/x-www-form-urlencoded`)
  — always hoist their fields to top-level; binary fields surface as
  `Blob | File`. The generated builder materializes them into
  `FormData` / `URLSearchParams` at runtime.
- **Scalar/array JSON bodies** — kept nested as `body: T` (there is no
  property structure to hoist).

```ts
// `updatePet` — body is `$ref: UpdatePetRequest`, so it nests.
export interface UpdatePetParams {
  petId: PetId;             // path param
  includeHistory?: boolean; // query param
  body: UpdatePetRequest;   // ref body, nested
}

// `decide` — body is an inline `{ csvImportId, doImport }` object, so
// its properties hoist to top-level.
export interface DecideParams {
  csvImportId: CsvImportId;
  doImport: boolean;
}
```

The escape hatch in either direction is in the spec: hoist an inline
body to `components/schemas` (give it a name) to switch to a nested
`body`, or inline a named schema's contents at the operation site to
hoist its properties. Hoisted properties that collide with a path or
query parameter name are rejected at codegen with
`E_POLICY_VIOLATION` / `field-collision` — rename the offender or
hoist the body schema to a `$ref` so it nests under `body` instead.

## Configuring the base path

`rest.util.ts` exports an `OPENAPI_NG_BASE_PATH` injection token and a
matching `provideOpenapiNg` helper. Provide it once at bootstrap and
every service prepends it to the generated relative URL automatically
(the prefix is joined with a single `/`, regardless of trailing/leading
slashes):

```ts
import { ApplicationConfig } from '@angular/core';
import { provideHttpClient } from '@angular/common/http';
import { provideOpenapiNg } from './generated/rest.util';

export const appConfig: ApplicationConfig = {
  providers: [
    provideHttpClient(),
    provideOpenapiNg({ basePath: 'https://api.example.com' }),
  ],
};
```

Skip the provider and requests fall back to the spec-relative URL
(`/pets`, `/pets/{id}`, …) — useful in dev with a proxy.

## `.resource()` — typed `httpResource`

Generated services delegate to Angular's
[`httpResource`](https://angular.dev/api/common/http/httpResource), so
**`.resource()` accepts every option `httpResource` accepts** — request
transforms, equality, injector, and so on. The generator only adds
strong typing on top.

The signature uses overloads keyed on whether you pass `defaultValue`
and/or `parse`:

| Options                       | Return type                       |
|-------------------------------|-----------------------------------|
| *(none)*                      | `HttpResourceRef<T \| undefined>` |
| `{ defaultValue }`            | `HttpResourceRef<T>`              |
| `{ parse }`                   | `HttpResourceRef<U \| undefined>` |
| `{ defaultValue, parse }`     | `HttpResourceRef<U>`              |

…where `T` is the spec-declared response type and `U` is whatever
`parse` returns.

### Why `parse` matters here

Angular's stock `httpResource<TResult>(req, { parse })` types `parse`
as `(raw: unknown) => TResult` — you take an `unknown` blob and prove
it's `TResult`. The generated `.resource()` does better: **`raw` is
typed as the spec's response type**, so `parse` becomes an honest
transformation rather than a runtime cast.

```ts
import { PetRest } from './generated/rest/pet.rest.generated';
import type { Pet } from './generated/model.generated';

@Component({ /* ... */ })
export class PetList {
  readonly #petRest = inject(PetRest);

  // raw: Pet, return: PetSummary — both fully typed, no `as` needed.
  protected readonly summary =
    this.#petRest.getPet.resource(
      () => ({ petId: this.selectedId() }),
      {
        defaultValue: { id: '', label: '—' } satisfies PetSummary,
        parse: (raw) => ({ id: raw.id, label: raw.name }),
      },
    );
}

interface PetSummary { id: string; label: string }
```

Pass any of the standard `httpResource` options the same way — for
example a `defaultValue`, an `equal` comparator, or an `injector`:

```ts
this.#petRest.listPets.resource({
  defaultValue: [],
  equal: (a, b) => a.length === b.length,
});
```

### Reactive request gating

The reactive request callback may return `undefined` to skip the call
(matching `httpResource`'s convention):

```ts
this.#petRest.getPet.resource(
  () => this.selectedId() ? { petId: this.selectedId() } : undefined,
);
```

### Response metadata via the ref

`HttpResourceRef` exposes the response envelope as Angular signals on
the returned ref — no observe/response juggling required:

```ts
protected readonly petResource =
  this.#petRest.getPet.resource(() => ({ petId: this.selectedId() }));

// Read inside an effect, computed, or template binding:
this.petResource.headers();      // Signal<HttpHeaders | undefined>
this.petResource.statusCode();   // Signal<number | undefined>
this.petResource.progress();     // Signal<HttpProgressEvent | undefined>
```

Use these for download progress UI, status-code-driven branching, or
reading caching/correlation headers without dropping out of the
signal-based flow.

## `.observable()` modes

`.observable(req)` returns `Observable<T>` by default — the response
body, decoded according to the operation's content type. Pass an
options object to switch observation modes or to forward extra
`HttpClient.request` configuration:

```ts
this.#petRest.updatePet.observable(req);
this.#petRest.updatePet.observable(req, { observe: 'response' });
this.#petRest.updatePet.observable(req, { observe: 'events', reportProgress: true });
```

| Options                       | Return type                       |
|-------------------------------|-----------------------------------|
| *(none)* / `{ observe: 'body' }`    | `Observable<T>`                     |
| `{ observe: 'response' }`           | `Observable<HttpResponse<T>>`       |
| `{ observe: 'events' }`             | `Observable<HttpEvent<T>>`          |

The options bag mirrors `HttpClient.request`'s options minus the
fields the generator already supplies — `body`, `params`, `headers`,
and `responseType` are baked in from the operation, so the type
rejects them. Everything else is forwarded: `withCredentials`,
`reportProgress`, `transferCache`, `context`, `keepalive`, and the
Fetch-related options (`redirect`, `mode`, `credentials`, `priority`,
`cache`, `timeout`).

For void-response operations (204 No Content), the same overloads
still apply — `Observable<HttpResponse<void>>` is meaningful when you
need `Location`, `ETag`, or trace headers from a `POST` / `PUT` /
`DELETE`.

## Using a service

```ts
@Component({ /* ... */ })
export class PetList {
  readonly #petRest = inject(PetRest);

  // As an Observable
  protected readonly pets$ =
    this.#petRest.listPets.observable();

  // As an HttpResource (reactive, signal-based)
  protected readonly petsResource =
    this.#petRest.listPets.resource({
      defaultValue: [],
    });

  // With parameters
  protected readonly petResource =
    this.#petRest.getPet.resource(
      () => ({ petId: this.selectedId() }),
    );

  // Imperative call
  protected update(petId: PetId) {
    this.#petRest.updatePet.observable({
      petId,
      status: 'sold',
      tagIds: [1, 2],
    }).subscribe();
  }

  // Raw CommonRequest — for custom transports, logging, etc.
  protected debug(petId: PetId) {
    const req = this.#petRest.getPet.request({ petId });
    console.log(req.method, req.url, req.params);
  }
}
```

## Non-JSON responses

When an operation declares a response content type other than JSON,
the generator emits the same three flavors but routes them through
the matching `httpResource` factory. The `requestFactory` symbol
itself is a callable for JSON, with sibling factories for the other
kinds:

| Response kind | Factory                       | Emitted return type           |
|---------------|-------------------------------|-------------------------------|
| `json`        | `requestFactory(...)`         | `Observable<T>` / `HttpResourceRef<T>` |
| `blob`        | `requestFactory.blob(...)`    | `Observable<Blob>` / `HttpResourceRef<Blob>` |
| `text`        | `requestFactory.text(...)`    | `Observable<string>` / `HttpResourceRef<string>` |
| `arrayBuffer` | `requestFactory.arrayBuffer(...)` | `Observable<ArrayBuffer>` / `HttpResourceRef<ArrayBuffer>` |

The picker uses the response's declared content type; you can override
the mapping per content type with the `responseTypeMapping` option (CLI:
not currently exposed; Node: `generate({ responseTypeMapping: [...] })`).

## What `rest.model.ts` / `rest.util.ts` ship

The generator emits two hand-written helper files alongside the
service. They are stable, dependency-free, and worth knowing because
your code can import from them directly.

### `rest.model.ts`

Type-only module. Defines:

- `QueryParamValue` — the value types `httpParams` accepts
  (`string | number | boolean | ReadonlyArray<…>`).
- `CommonRequest` — the raw request returned by `.request()`. Extends
  Angular's `HttpResourceRequest` and adds `method` / `url`. Use this
  for custom transports.
- `WithDefault<T>`, `WithParse<T, TRaw>` — mixin shapes for resource
  options.
- `BaseHttpResourceOptions<T, TRaw>` — `HttpResourceOptions` with
  `parse` and `defaultValue` stripped out (so the generator can put
  back better-typed versions).
- `BaseHttpResourceOptionsWithDefault<T, TRaw>` /
  `…WithParse<T, TRaw>` / `…WithDefaultAndParse<T, TRaw>` — the four
  permutations consumed by the `.resource()` overloads.
- `HttpResourceOptionsUnion<T, TRaw>` — union over all four.

### `rest.util.ts`

Runtime module. Exports:

- `OPENAPI_NG_BASE_PATH` — `InjectionToken<string>` for the API base.
- `provideOpenapiNg({ basePath })` — `EnvironmentProviders` shortcut.
- `httpParams(record)` — builds an `HttpParams` from a record,
  skipping `undefined` and flattening arrays (each item becomes a
  repeated query param).
- `RequestFn<Request, Response>` / `ZeroArgRequestFn<Response>` — the
  shape of every generated operation property (generic parameters are
  `Request` first, `Response` second); `RequestFnVoid<Request>` /
  `ZeroArgRequestFnVoid` are the void-response variants picked
  automatically when `Response` is `void`.
- `requestFactory` — callable for JSON; `.blob` / `.text` /
  `.arrayBuffer` cover the non-JSON response kinds.

## Assumptions and limitations

`openapi-ng` accepts a focused subset of OpenAPI 3.x. See the
[Limitations reference](/reference/limitations/) for the full list of
required fields, supported schema shapes, and what's out of scope.
