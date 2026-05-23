---
title: Runtime & platforms
description: Supported runtimes and platforms — Node, Bun, Deno via N-API. Browser and edge runtimes are not supported.
---

This page covers the runtimes that can host the **`openapi-ng`
generator** (the CLI / Node API that turns an OpenAPI spec into
TypeScript). For the runtime helpers shipped *inside* generated
output (`rest.util.ts` / `rest.model.ts`), see the [Angular generator
guide](/guides/angular/#what-rest-model-ts--rest-util-ts-ship).

## Primary runtime

`openapi-ng` targets Node.js as its primary runtime: the generation
engine is a Rust binary loaded via [NAPI-RS](https://napi.rs).

Requires Node.js 18+.

Bun and Deno are supported on the same native path because both
implement N-API and pick up the prebuilt `.node` artifact directly.

## Platforms

Pre-built native binaries are published for:

- macOS (x64, ARM64)
- Linux (x64, ARM64)
- Windows (x64, ARM64)

On any other platform (musl Linux, FreeBSD, 32-bit, etc.), `require`
of the package throws an explicit unsupported-platform error listing
the supported set — open an issue if you need an additional target.

## Browser and edge runtimes

`openapi-ng` does not support browser runtimes, Cloudflare Workers,
Vercel Edge, Deno Deploy, or any other host that cannot load a native
N-API binary. The package ships a `browser.js` stub that any
browser-aware bundler (Vite, Webpack, esbuild) will resolve in those
contexts; calling `generate()` from it throws a `GenerateError` with
code `E_UNSUPPORTED_RUNTIME`.

Code generation is a build-time activity — run it from a Node script,
not from a browser bundle. See
[`E_UNSUPPORTED_RUNTIME`](/reference/diagnostics/#diagnostic-codes)
for the error shape.
