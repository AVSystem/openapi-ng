---
title: Runtime & platforms
description: Supported runtimes and platforms — Node, Bun, Deno via N-API, and the WASI path for hosts without N-API.
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

Today `openapi-ng` does not support browser runtimes — the package
ships a `browser.js` stub that throws a clear error
(`E_UNSUPPORTED_RUNTIME`) if a bundler ever resolves the package in a
browser target. The browser stub is a thin transport over the same
native `generate()` path used by Node — the public surface is
identical, but the browser entry point only ever throws because there
is no in-browser implementation of the native engine. See [Non-Node
runtimes](#non-node-runtimes) below for the WASM story that will
eventually unlock browsers and edge hosts.

## Platforms

Pre-built native binaries are published for:

- macOS (x64, ARM64)
- Linux (x64, ARM64)
- Windows (x64, ARM64)

## Non-Node runtimes

For hosts without N-API support (browsers, Cloudflare Workers, Vercel
Edge), a WASI build is published as the
`@avsystem/openapi-ng-wasm32-wasip1-threads` optional dependency. The Node
loader at `lib/index.js` (via `native.js`) falls through to this WASM
artifact automatically when no platform-specific `.node` is available,
so Node itself stays covered on architectures without a prebuilt
binary.

- **Bun** and **Deno** are already supported via N-API natively. They
  take the `.node` path and do not need the WASM artifact.
- **Browsers** (via build tools like Vite/Webpack that route to
  `browser` exports today) — *not yet supported*; the current
  `browser.js` stub throws. A future release will route browser
  targets through `@avsystem/openapi-ng-wasm32-wasip1-threads` loaded with
  `@napi-rs/wasm-runtime`.
- **Cloudflare Workers / Vercel Edge** — likewise pending a small
  browser-side loader; TBD in the 1.0 roadmap.

The WASM artifact ships as an optional dep, so consumers who only need
the native binary do not pay for the download.
