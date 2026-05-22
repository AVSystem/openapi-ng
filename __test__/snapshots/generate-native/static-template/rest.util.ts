import { HttpClient, HttpParams, httpResource } from '@angular/common/http';
import type {
  HttpEvent,
  HttpResourceOptions,
  HttpResourceRef,
  HttpResponse,
} from '@angular/common/http';
import {
  InjectionToken,
  inject,
  makeEnvironmentProviders,
  type EnvironmentProviders,
} from '@angular/core';
import type { Observable } from 'rxjs';
import type {
  BaseHttpResourceOptionsWithDefault,
  BaseHttpResourceOptionsWithDefaultAndParse,
  BaseHttpResourceOptionsWithParse,
  CommonRequest,
  HttpResourceOptionsUnion,
  ObservableOptions,
  QueryParamValue,
} from './rest.model';

type QueryParamRecord = Record<string, QueryParamValue | undefined>;
type ResponseType = 'blob' | 'text' | 'arraybuffer';

export const OPENAPI_NG_BASE_PATH = new InjectionToken<string>(
  'OPENAPI_NG_BASE_PATH',
);

export function provideOpenapiNg(config: {
  basePath: string;
}): EnvironmentProviders {
  return makeEnvironmentProviders([
    { provide: OPENAPI_NG_BASE_PATH, useValue: config.basePath },
  ]);
}

export function httpParams(params: QueryParamRecord): HttpParams {
  let resolved = new HttpParams();
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined) {
      if (Array.isArray(value)) {
        for (const item of value) {
          resolved = resolved.append(key, String(item));
        }
      } else {
        resolved = resolved.set(key, String(value));
      }
    }
  }
  return resolved;
}

export interface RequestFnValue<Request, Response> {
  request(request: Request): CommonRequest;
  observable(
    request: Request,
    options: ObservableOptions & { observe: 'response' },
  ): Observable<HttpResponse<Response>>;
  observable(
    request: Request,
    options: ObservableOptions & { observe: 'events' },
  ): Observable<HttpEvent<Response>>;
  observable(request: Request, options?: ObservableOptions): Observable<Response>;
  resource(
    reactiveReq: () => Request | undefined,
    options: BaseHttpResourceOptionsWithDefault<Response>,
  ): HttpResourceRef<Response>;
  resource<TResult>(
    reactiveReq: () => Request | undefined,
    options: BaseHttpResourceOptionsWithDefaultAndParse<TResult, Response>,
  ): HttpResourceRef<TResult>;
  resource(
    reactiveReq: () => Request | undefined,
    options?: HttpResourceOptionsUnion<Response, Response>,
  ): HttpResourceRef<Response | undefined>;
  resource<TResult>(
    reactiveReq: () => Request | undefined,
    options: BaseHttpResourceOptionsWithParse<TResult, Response>,
  ): HttpResourceRef<TResult | undefined>;
}

export interface RequestFnVoid<Request> {
  request(request: Request): CommonRequest;
  observable(
    request: Request,
    options: ObservableOptions & { observe: 'response' },
  ): Observable<HttpResponse<void>>;
  observable(
    request: Request,
    options: ObservableOptions & { observe: 'events' },
  ): Observable<HttpEvent<void>>;
  observable(request: Request, options?: ObservableOptions): Observable<void>;
  resource(
    reactiveReq: () => Request | undefined,
    options?: HttpResourceOptionsUnion<void, unknown>,
  ): HttpResourceRef<void>;
}

export type RequestFn<Request, Response> = [Response] extends [void]
  ? RequestFnVoid<Request>
  : RequestFnValue<Request, Response>;

export interface ZeroArgRequestFnValue<Response> {
  request(): CommonRequest;
  observable(
    options: ObservableOptions & { observe: 'response' },
  ): Observable<HttpResponse<Response>>;
  observable(
    options: ObservableOptions & { observe: 'events' },
  ): Observable<HttpEvent<Response>>;
  observable(options?: ObservableOptions): Observable<Response>;
  resource(
    options: BaseHttpResourceOptionsWithDefault<Response>,
  ): HttpResourceRef<Response>;
  resource<TResult>(
    options: BaseHttpResourceOptionsWithDefaultAndParse<TResult, Response>,
  ): HttpResourceRef<TResult>;
  resource(
    options?: HttpResourceOptionsUnion<Response, Response>,
  ): HttpResourceRef<Response | undefined>;
  resource<TResult>(
    options: BaseHttpResourceOptionsWithParse<TResult, Response>,
  ): HttpResourceRef<TResult | undefined>;
}

export interface ZeroArgRequestFnVoid {
  request(): CommonRequest;
  observable(
    options: ObservableOptions & { observe: 'response' },
  ): Observable<HttpResponse<void>>;
  observable(
    options: ObservableOptions & { observe: 'events' },
  ): Observable<HttpEvent<void>>;
  observable(options?: ObservableOptions): Observable<void>;
  resource(options?: HttpResourceOptionsUnion<void, unknown>): HttpResourceRef<void>;
}

export type ZeroArgRequestFn<Response> = [Response] extends [void]
  ? ZeroArgRequestFnVoid
  : ZeroArgRequestFnValue<Response>;

function joinBasePath(base: string, url: string): string {
  // Absolute URLs (https://…, etc.) bypass the configured basePath.
  if (/^[a-z][a-z0-9+.-]*:\/\//i.test(url)) return url;
  const normalizedBase = base.endsWith('/') ? base.slice(0, -1) : base;
  const normalizedUrl = url.startsWith('/') ? url : `/${url}`;
  return normalizedBase + normalizedUrl;
}

function withBasePath<TArgs extends readonly unknown[]>(
  reqFn: (...args: TArgs) => CommonRequest,
): (...args: TArgs) => CommonRequest {
  const basePath = inject(OPENAPI_NG_BASE_PATH, { optional: true });
  if (!basePath) return reqFn;
  return (...args: TArgs) => {
    const common = reqFn(...args);
    return { ...common, url: joinBasePath(basePath, common.url) };
  };
}

type ObserveFn<TResult> = (
  request: CommonRequest,
  options?: ObservableOptions,
) => Observable<TResult>;

function makeObserveFn<TResult>(
  http: HttpClient,
  responseType?: ResponseType,
): ObserveFn<TResult> {
  return (request, options) => {
    const merged = {
      ...options,
      body: request.body,
      headers: request.headers,
      params: request.params,
      ...(responseType ? { responseType } : {}),
    };
    return http.request(request.method, request.url, merged) as Observable<TResult>;
  };
}

function makeRequestFn<Request, TResult, TRaw>(
  reqFn: (req: Request) => CommonRequest,
  observe: ObserveFn<TResult>,
  resourceImpl: (
    request: () => CommonRequest | undefined,
    options?: HttpResourceOptions<TResult, TRaw>,
  ) => HttpResourceRef<TResult | undefined>,
): RequestFn<Request, TResult> {
  const wrappedReqFn = withBasePath(reqFn);
  return {
    request: (req: Request): CommonRequest => wrappedReqFn(req),
    observable: (req: Request, options?: ObservableOptions): Observable<TResult> =>
      observe(wrappedReqFn(req), options),
    resource: (
      reactiveReq: () => Request | undefined,
      options?: HttpResourceOptionsUnion<TResult, TRaw>,
    ): HttpResourceRef<TResult | undefined> =>
      resourceImpl(
        () => {
          const request = reactiveReq();
          return request === undefined ? undefined : wrappedReqFn(request);
        },
        options,
      ),
  } as RequestFn<Request, TResult>;
}

function makeZeroArgRequestFn<TResult, TRaw>(
  reqFn: () => CommonRequest,
  observe: ObserveFn<TResult>,
  resourceImpl: (
    request: () => CommonRequest | undefined,
    options?: HttpResourceOptions<TResult, TRaw>,
  ) => HttpResourceRef<TResult | undefined>,
): ZeroArgRequestFn<TResult> {
  const wrappedReqFn = withBasePath(reqFn);
  return {
    request: (): CommonRequest => wrappedReqFn(),
    observable: (options?: ObservableOptions): Observable<TResult> =>
      observe(wrappedReqFn(), options),
    resource: (
      options?: HttpResourceOptionsUnion<TResult, TRaw>,
    ): HttpResourceRef<TResult | undefined> =>
      resourceImpl(() => wrappedReqFn(), options),
  } as ZeroArgRequestFn<TResult>;
}

function makeJsonRequestFn<Request, Response>(
  reqFn: (request: Request) => CommonRequest,
): RequestFn<Request, Response> {
  const http = inject(HttpClient);
  return makeRequestFn<Request, Response, unknown>(
    reqFn,
    makeObserveFn<Response>(http),
    (request, options) => httpResource<Response>(request, options),
  );
}

function makeJsonZeroArg<Response>(
  reqFn: () => CommonRequest,
): ZeroArgRequestFn<Response> {
  const http = inject(HttpClient);
  return makeZeroArgRequestFn<Response, unknown>(
    reqFn,
    makeObserveFn<Response>(http),
    (request, options) => httpResource<Response>(request, options),
  );
}

function makeBlobRequestFn<Request>(
  reqFn: (request: Request) => CommonRequest,
): RequestFn<Request, Blob> {
  const http = inject(HttpClient);
  return makeRequestFn<Request, Blob, Blob>(
    reqFn,
    makeObserveFn<Blob>(http, 'blob'),
    (request, options) => httpResource.blob(request, options),
  );
}

function makeBlobZeroArg(reqFn: () => CommonRequest): ZeroArgRequestFn<Blob> {
  const http = inject(HttpClient);
  return makeZeroArgRequestFn<Blob, Blob>(
    reqFn,
    makeObserveFn<Blob>(http, 'blob'),
    (request, options) => httpResource.blob(request, options),
  );
}

function makeTextRequestFn<Request>(
  reqFn: (request: Request) => CommonRequest,
): RequestFn<Request, string> {
  const http = inject(HttpClient);
  return makeRequestFn<Request, string, string>(
    reqFn,
    makeObserveFn<string>(http, 'text'),
    (request, options) => httpResource.text(request, options),
  );
}

function makeTextZeroArg(reqFn: () => CommonRequest): ZeroArgRequestFn<string> {
  const http = inject(HttpClient);
  return makeZeroArgRequestFn<string, string>(
    reqFn,
    makeObserveFn<string>(http, 'text'),
    (request, options) => httpResource.text(request, options),
  );
}

function makeArrayBufferRequestFn<Request>(
  reqFn: (request: Request) => CommonRequest,
): RequestFn<Request, ArrayBuffer> {
  const http = inject(HttpClient);
  return makeRequestFn<Request, ArrayBuffer, ArrayBuffer>(
    reqFn,
    makeObserveFn<ArrayBuffer>(http, 'arraybuffer'),
    (request, options) => httpResource.arrayBuffer(request, options),
  );
}

function makeArrayBufferZeroArg(
  reqFn: () => CommonRequest,
): ZeroArgRequestFn<ArrayBuffer> {
  const http = inject(HttpClient);
  return makeZeroArgRequestFn<ArrayBuffer, ArrayBuffer>(
    reqFn,
    makeObserveFn<ArrayBuffer>(http, 'arraybuffer'),
    (request, options) => httpResource.arrayBuffer(request, options),
  );
}

export const requestFactory = Object.assign(makeJsonRequestFn, {
  blob: makeBlobRequestFn,
  text: makeTextRequestFn,
  arrayBuffer: makeArrayBufferRequestFn,
  zeroArg: Object.assign(makeJsonZeroArg, {
    blob: makeBlobZeroArg,
    text: makeTextZeroArg,
    arrayBuffer: makeArrayBufferZeroArg,
  }),
});
