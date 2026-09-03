# Title options

1. I built an open-source OpenAPI → Angular generator because none gave me httpResource and Observables on the same operation. Comparison inside.
2. Yet another OpenAPI → Angular generator (MIT). Here's why I built one anyway, and how it compares.

---

I know. Another OpenAPI generator for Angular. Before you scroll past, here's the honest version of why I built one at work and open-sourced it, and how it compares to what already exists.

**The problem I hit**

I wanted three things from generated clients and couldn't get them from one tool:

1. `httpResource()` and `HttpClient` Observables for the *same* operation, chosen at the call site, not at config time.
2. Every `HttpClient.request` / `httpResource` option passed straight through (`withCredentials`, `transferCache`, `reportProgress`, `equal`, `injector`, `observe: 'events'`...), without the generator wrapping or hiding anything.
3. Zero runtime dependency in the generated code. Just `@angular/common/http`.

So: https://github.com/AVSystem/openapi-ng (MIT). Live demo, no install: Playground (paste your spec, runs as WASM in the browser): https://docs.openapi-ng.dev/playground/ · Angular app demo: https://stackblitz.com/github/AVSystem/openapi-ng/tree/main/stackblitz

**Before / after**

What I kept hand-writing:

```ts
readonly pets = httpResource<Pet[]>(() => ({
  url: `${this.basePath}/pets`,
  params: this.status() ? { status: this.status()! } : {},
}), { defaultValue: [] });
```

The URL, the query param and the `<Pet[]>` are all promises I make to the compiler. Nothing checks them against the spec.

Generated:

```ts
readonly #petRest = inject(PetRest);

// signal-based, httpResource under the hood, typed HttpResourceRef<PetList>
readonly pets = this.#petRest.listPets.resource(() => ({ status: this.status() }), { defaultValue: [] });

// same operation, classic Observable, HttpClient under the hood
save(petId: PetId, body: UpdatePetRequest) {
  return this.#petRest.updatePet.observable({ petId, body }, { observe: 'response' });
}
```

Each tag becomes an `@Injectable` service, each operation a property with `.resource()`, `.observable()` and `.request()` (the raw `{ method, url, params, body }` if you want to send it yourself). `.resource()` returns a real `HttpResourceRef<T>`, so `headers()`, `statusCode()` and `progress()` are there. Nothing is re-wrapped.

**Signal forms: async validation against a generated operation**

With signal forms in v22, the thing I kept hand-writing was the "is this email taken" validator with `validateHttp`: URL string by hand, response cast by hand.

```ts
validateHttp(path.email, {
  request: (ctx) => `${base}/accounts/check-email?email=${encodeURIComponent(ctx.value())}`,
  onSuccess: (result) => (result as EmailAvailability).available ? undefined : { kind: 'email-taken' },
  onError: () => ({ kind: 'check-failed' }),
});
```

The generator emits a `validateRest()` helper that wraps `validateAsync` and reuses the generated operation:

```ts
validateRest(path.email, accountRest.checkEmail, {
  request: (ctx) => ({ email: ctx.value() }), // typed as CheckEmailParams
  debounce: 300,
  when: (ctx) => ctx.value().includes('@'),
  onSuccess: (result) => result.available ? undefined : { kind: 'email-taken' }, // result: EmailAvailability
  onError: () => ({ kind: 'check-failed' }),
});
```

Omit `onSuccess` for the "200 means fine, 4xx means taken" shape. `debounce` and `when` go straight to Angular's `validateAsync`. `@angular/forms` is an optional peer; if you never import `rest.validate.ts`, it tree-shakes away.

**How it compares (checked 2026-09-03, corrections welcome)**

| | openapi-ng | openapi-generator | ng-openapi-gen | orval | ng-openapi |
|---|---|---|---|---|---|
| httpResource | every operation, per call | no ([#21263](https://github.com/OpenAPITools/openapi-generator/issues/21263) open) | no | GET only, config mode | GET only, plugin |
| Observable and resource on one operation | yes | n/a | n/a | separate files | separate classes |
| Signal-forms validation helper | yes | no | no | no | no |
| Unsupported spec features | rejected with a diagnostic code | best-effort | best-effort | best-effort | best-effort |
| Engine | Rust (napi-rs), ms per spec, also runs in the browser via WASM | Java | TS | TS | TS |

Two takeaways:

- If you're on openapi-generator today, the Java requirement and the lack of httpResource are probably the two things that sting. openapi-ng does both.
- orval and ng-openapi both have httpResource now, but only for GET and the flavour is picked per config, not per call. I think the call site knows better (a `POST /search` is a perfectly good resource; a GET that fires a side effect isn't).

**Where it's deliberately opinionated (read: limited)**

This is the part most "yet another generator" posts skip. openapi-ng accepts a *strict* subset of OpenAPI 3.x and refuses anything outside it with a diagnostic code instead of guessing:

- `operationId` and at least one `tag` are required on every operation
- only `#/components/schemas/` refs, no external files
- string enums only
- one composition keyword per schema (`allOf` or `oneOf` or `anyOf`, not mixed)
- only the lowest 2xx response is typed. 4xx/5xx types aren't generated at all
- no Zod or other runtime validation, no Swagger 2.x

If your spec is generated from a backend framework, you're probably fine. If it's a hand-written spec that leans on every corner of the standard, one of the other tools above will be a better fit and I'd rather tell you now.

Escape hatches for the real-world stuff: mapped types (`--mapped-type GeoFeature:geojson:Feature` swaps a placeholder schema for a real npm type), naming rules with regex captures in `openapi-ng.config.ts`, and a `provideOpenapiNg({ basePath })` provider.

**Links and bits**

- Playground: https://docs.openapi-ng.dev/playground/
- Demo: https://stackblitz.com/github/AVSystem/openapi-ng/tree/main/stackblitz (the spec, the committed generated output, and three components using it against a mock interceptor)
- Docs: https://docs.openapi-ng.dev
- `npm i -D @avsystem/openapi-ng`, then `openapi-ng generate --input api.yaml --output src/generated`
- Node 22.12+, prebuilt binaries for macOS / Linux (glibc + musl) / Windows, x64 + arm64. Works under Bun and Deno too. Browser via WebAssembly (@avsystem/openapi-ng-wasm32-wasi).
- Generated code targets Angular 20+. `validateRest` with `debounce` / `when` needs `@angular/forms` 22+.
- v0.5.1. I use it in production at work but the public API isn't frozen yet.

**What I'd like feedback on**

Two design calls I'm least sure about:

1. One service per tag with `.resource()` / `.observable()` / `.request()` on every operation, versus orval's approach of separate generated classes per flavour. Is the per-call choice worth the fatter service surface?
2. Rejecting unsupported spec features with a diagnostic instead of emitting best-effort code. Has a strict generator ever made you *drop* a tool, or is it the right trade for deterministic output?

Happy to answer anything else in the thread.
