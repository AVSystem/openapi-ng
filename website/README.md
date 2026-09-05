# openapi-ng website

Astro Starlight site behind [docs.openapi-ng.dev](https://docs.openapi-ng.dev/): the
documentation under `src/content/docs/` and the browser playground under `src/playground/`.

```bash
pnpm install
pnpm --filter @avsystem/openapi-ng-website dev      # local dev server
pnpm --filter @avsystem/openapi-ng-website build    # production build
pnpm --filter @avsystem/openapi-ng-website test     # vitest + playwright
```

## Running the playground against the local build

The site pins the published `@avsystem/openapi-ng` and `@avsystem/openapi-ng-wasm32-wasi`
packages in `package.json`, and `scripts/bundle-engine.mjs` copies the wasm loader out of
`node_modules` on every `dev`/`build`. To try unreleased generator changes in the playground,
link both packages to the local build. From the repo root:

```bash
pnpm build --target wasm32-wasip1-threads   # wasm engine; writes openapi-ng.wasi* and *.wasm at the root
git checkout native.js                      # the build rewrites the committed entry file
pnpm napi create-npm-dirs                   # npm/<target>/package.json for every target (gitignored)
cp openapi-ng.wasm32-wasi.wasm openapi-ng.wasi.cjs openapi-ng.wasi.d.cts \
   openapi-ng.wasi-browser.js wasi-worker.mjs wasi-worker-browser.mjs npm/wasm32-wasi/
```

Then in `website/package.json` set

```json
"@avsystem/openapi-ng": "workspace:*",
"@avsystem/openapi-ng-wasm32-wasi": "link:../npm/wasm32-wasi"
```

and run `pnpm install && pnpm --filter @avsystem/openapi-ng-website dev`. Both packages must be
linked: the JS wrapper validates option keys, so a published wrapper rejects options the local
engine adds. After further Rust or template changes, rebuild the wasm, copy the six files again
and restart `dev` so `predev` re-bundles the engine.

Before committing, revert `package.json` and the root `pnpm-lock.yaml`. Bump the pins only once
the new version is on npm.
