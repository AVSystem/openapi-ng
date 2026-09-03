---
title: Runtime & platforms
description: Supported runtimes and platforms — Node, Bun, Deno via N-API; browsers via WebAssembly.
---

This page covers the runtimes that can host the **`openapi-ng`
generator** (the CLI / Node API that turns an OpenAPI spec into
TypeScript). For the runtime helpers shipped *inside* generated
output (`rest.util.ts` / `rest.model.ts`), see the [Angular generator
guide](/guides/angular/#what-rest-model-ts--rest-util-ts-ship).

## Primary runtime

`openapi-ng` targets Node.js as its primary runtime: the generation
engine is a Rust binary loaded via [NAPI-RS](https://napi.rs).

Requires Node.js 22.12+.

Bun and Deno are supported on the same native path because both
implement N-API and pick up the prebuilt `.node` artifact directly.

## Platforms

Pre-built native binaries are published for:

- macOS (x64, ARM64)
- Linux glibc (x64, ARM64)
- Linux musl (x64, ARM64) — Alpine and other musl-based images
- Windows (x64, ARM64)

The correct glibc or musl artifact is selected at load time, so Alpine
and distroless images need no extra configuration.

On any other platform (FreeBSD, 32-bit, etc.) the loader falls back to
the WebAssembly build if `@avsystem/openapi-ng-wasm32-wasi` is installed
next to `@avsystem/openapi-ng`; otherwise `require` throws an explicit
unsupported-platform error listing the supported set.

## Browsers

The generator also ships as WebAssembly in the platform package
`@avsystem/openapi-ng-wasm32-wasi`. The `browser` entry of
`@avsystem/openapi-ng` loads it on demand, so bundlers that honour the
`browser` field (Vite, webpack, esbuild) pick the WebAssembly path
automatically.

Two requirements:

1. Add the platform package yourself. napi-rs does not list it among the
   root package's optional dependencies, so it is not installed
   transitively:

   ```bash
   pnpm add -D @avsystem/openapi-ng @avsystem/openapi-ng-wasm32-wasi
   ```

   Import `createGenerate` from `@avsystem/openapi-ng/browser` when you
   serve the WebAssembly files yourself.

2. Serve the page cross-origin isolated. The binding uses shared memory,
   which browsers only enable under these response headers:

   ```
   Cross-Origin-Opener-Policy: same-origin
   Cross-Origin-Embedder-Policy: require-corp
   ```

If the package is missing, bundlers may warn about an unresolvable
dynamic import and `generate()` rejects with `E_UNSUPPORTED_RUNTIME` at
call time.

In the browser, `generate()` accepts `inputContents` plus `displayPath`
and returns the artifacts in memory. `inputPath` and `outputPath` are
rejected with `E_INVALID_OPTION` because there is no filesystem. If the
WebAssembly binding cannot be loaded, `generate()` rejects with
`E_UNSUPPORTED_RUNTIME` and a message naming the missing package or
header.

## Edge runtimes

Cloudflare Workers, Vercel Edge and Deno Deploy are not verified. The
WebAssembly build may run there; open an issue with your findings if you
try it.
