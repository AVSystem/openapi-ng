# openapi-ng website

Astro Starlight site behind [docs.openapi-ng.dev](https://docs.openapi-ng.dev/): the
documentation under `src/content/docs/` and the browser playground under `src/playground/`.

```bash
bun install
bun run --filter @avsystem/openapi-ng-website dev      # local dev server
bun run --filter @avsystem/openapi-ng-website build    # production build
bun run --filter @avsystem/openapi-ng-website test     # vitest + playwright
```

## The playground engine

The playground runs the generator from this checkout, not a published package:
`astro.config.mjs` aliases `@avsystem/openapi-ng/browser` to the root `lib/browser.js`, and
`scripts/bundle-engine.mjs` copies the wasm loader from the repo root on every `dev`/`build`.
Build the engine once before starting the site, and again after any Rust or template change:

```bash
bun run build --target wasm32-wasip1-threads   # writes openapi-ng.wasi* and *.wasm at the root
git checkout -- native.js browser.js index.d.ts   # the build regenerates the committed entries
```

Restart `dev` afterwards so `predev` re-bundles the engine. The deployed site is built the same
way by `.github/workflows/docs.yml`, so a release commit ships the docs and playground for that
exact version; the workflow deploys to production only for a release commit or a docs-only
change and uploads a preview version for everything else.
