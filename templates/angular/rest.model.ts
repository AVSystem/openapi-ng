import type {
  HttpContext,
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

// Omits body/params/headers (generator supplies them) and responseType
// (fixed per requestFactory variant). Structurally compatible with
// HttpClient.request(method, url, options) so the runtime spread doesn't
// need a Parameters<…>[2] cast.
export type ObservableOptions = {
  context?: HttpContext;
  observe?: 'body' | 'response' | 'events';
  reportProgress?: boolean;
  transferCache?: { includeHeaders?: string[] } | boolean;
  withCredentials?: boolean;
  keepalive?: boolean;
  redirect?: RequestRedirect;
  mode?: RequestMode;
  credentials?: RequestCredentials;
  priority?: RequestPriority;
  cache?: RequestCache;
  timeout?: number;
};
