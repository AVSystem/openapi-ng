import type { Injector } from '@angular/core';
import type {
  HttpClient,
  HttpHeaders,
  HttpParams,
  HttpResourceOptions,
  HttpResourceRequest,
} from '@angular/common/http';

export type QueryParamValue =
  | string
  | number
  | boolean
  | ReadonlyArray<string | number | boolean>;

export interface CommonRequest extends Pick<
  HttpResourceRequest,
  'body' | 'params' | 'headers'
> {
  method: string;
  url: string;
  body?: unknown;
  params?: HttpParams | Record<string, QueryParamValue>;
  headers?: HttpHeaders | Record<string, string | string[]>;
}

export interface WithDefault<TResult> {
  defaultValue: NoInfer<TResult>;
}

export interface WithParse<TResult, TRaw> {
  parse: (raw: TRaw) => TResult;
}

export type BaseHttpResourceOptions<TResult, TRaw = TResult> = Omit<
  HttpResourceOptions<TResult, TRaw>,
  'parse' | 'defaultValue'
>;

export type BaseHttpResourceOptionsWithParse<TResult, TRaw> = BaseHttpResourceOptions<
  TResult,
  TRaw
> &
  WithParse<TResult, TRaw>;

export type BaseHttpResourceOptionsWithDefault<
  TResult,
  TRaw = TResult,
> = BaseHttpResourceOptions<TResult, TRaw> & WithDefault<TResult>;

export type BaseHttpResourceOptionsWithDefaultAndParse<TResult, TRaw> =
  BaseHttpResourceOptions<TResult, TRaw> &
    WithParse<TResult, TRaw> &
    WithDefault<TResult>;

export type HttpResourceOptionsUnion<TResult, TRaw = TResult> =
  | BaseHttpResourceOptions<TResult, TRaw>
  | BaseHttpResourceOptionsWithParse<TResult, TRaw>
  | BaseHttpResourceOptionsWithDefault<TResult, TRaw>
  | BaseHttpResourceOptionsWithDefaultAndParse<TResult, TRaw>;

// Standalone `.request()` reads the base path from `injector` when given and
// returns the spec-relative URL otherwise.
export interface OperationRequestOptions {
  injector?: Injector;
}

// `Parameters` reads request()'s last overload, its catch-all, so options a later
// Angular adds pass through. The operation supplies body/params/headers and fixes
// responseType; `injector` is consumed by the runtime and never reaches HttpClient.
export type ObservableOptions = Omit<
  NonNullable<Parameters<HttpClient['request']>[2]>,
  'body' | 'params' | 'headers' | 'responseType'
> & { injector?: Injector };
