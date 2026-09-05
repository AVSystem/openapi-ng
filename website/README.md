# openapi-ng website

Astro Starlight site behind [docs.openapi-ng.dev](https://docs.openapi-ng.dev/): the
documentation under `src/content/docs/` and the browser playground under `src/playground/`.

```bash
bun install
bun run --filter @avsystem/openapi-ng-website dev      # local dev server
bun run --filter @avsystem/openapi-ng-website build    # production build
bun run --filter @avsystem/openapi-ng-website test     # vitest + playwright
```

## Running the playground against the local build

The site pins the published `@avsystem/openapi-ng` and `@avsystem/openapi-ng-wasm32-wasi`
packages in `package.json`, and `scripts/bundle-engine.mjs` copies the wasm loader out of
`node_modules` on every `dev`/`build`. To try unreleased generator changes in the playground,
link both packages to the local build. From the repo root:

```bash
bun run build --target wasm32-wasip1-threads   # wasm engine; writes openapi-ng.wasi* and *.wasm at the root
git checkout native.js                      # the build rewrites the committed entry file
bunx napi create-npm-dirs                      # npm/<target>/package.json for every target (gitignored)
cp openapi-ng.wasm32-wasi.wasm openapi-ng.wasi.cjs openapi-ng.wasi.d.cts \
   openapi-ng.wasi-browser.js wasi-worker.mjs wasi-worker-browser.mjs npm/wasm32-wasi/
```

Then register both local packages with bun and point the site at them. Bun's `link:` protocol
refers to packages registered via `bun link` (a relative path there is not supported, and
`workspace:*` cannot name the workspace root):

```bash
bun link                                       # registers @avsystem/openapi-ng (repo root)
(cd npm/wasm32-wasi && bun link)               # registers @avsystem/openapi-ng-wasm32-wasi
```

In `website/package.json` set

```json
"@avsystem/openapi-ng": "link:@avsystem/openapi-ng",
"@avsystem/openapi-ng-wasm32-wasi": "link:@avsystem/openapi-ng-wasm32-wasi"
```

and run `bun install && bun run --filter @avsystem/openapi-ng-website dev`. Both packages must be
linked: the JS wrapper validates option keys, so a published wrapper rejects options the local
engine adds. The links are symlinks, so after further Rust or template changes just rebuild the
wasm, copy the six files again and restart `dev` so `predev` re-bundles the engine; no reinstall
is needed. Avoid `file:..` for the root package: bun copies `file:` dependencies, and that copies
the whole repo (including `target/`) into `node_modules`.

Before committing, revert `website/package.json` and the root `bun.lock`:

```bash
git checkout -- website/package.json bun.lock && bun install
```

Bump the pins only once the new version is on npm.
