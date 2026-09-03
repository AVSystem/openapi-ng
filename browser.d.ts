// Types for the browser entry (`browser.js` / `./browser` subpath).
import type { GenerateOptions, GenerateResult } from './index';

export type {
  DiagnosticCode,
  DiagnosticSubcode,
  GenerateErrorPayload,
  GeneratedArtifact,
  GenerateOptions,
  GenerateResult,
  GenerateSummary,
  GeneratorDiagnostic,
  InputFormat,
  MappedType,
  NamingConfig,
  ResponseTypeMapping,
} from './index';
export { EmitTarget, GenerateError } from './index';

/**
 * Generate in the browser. Same contract as the Node `generate`, except
 * `inputPath` and `outputPath` are rejected with `E_INVALID_OPTION`; pass
 * `inputContents` with `displayPath` and read `result.artifacts`.
 */
export declare function generate(options: GenerateOptions): Promise<GenerateResult>;

/**
 * Build a `generate` bound to a custom loader for the WebAssembly binding,
 * for hosts that serve the binding themselves instead of resolving
 * `@avsystem/openapi-ng-wasm32-wasi`. The loader resolves to the WASI
 * module namespace or its CommonJS `module.exports`; both expose
 * `generateNative`.
 */
export declare function createGenerate(
  loadBinding: () => Promise<unknown>,
): (options: GenerateOptions) => Promise<GenerateResult>;
