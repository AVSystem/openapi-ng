import { HttpClient, HttpParams, httpResource } from '@angular/common/http';
import type {
  HttpEvent,
  HttpResourceOptions,
  HttpResourceRef,
  HttpResponse,
} from '@angular/common/http';
import {
  InjectionToken,
  Injector,
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
  OperationRequestOptions,
  QueryParamValue,
} from './rest.model';

type QueryParamRecord = Record<string, QueryParamValue | undefined>;
type ResponseType = 'blob' | 'text' | 'arraybuffer';

export const OPENAPI_NG_BASE_PATH = new InjectionToken<string>('OPENAPI_NG_BASE_PATH');

export function provideOpenapiNg(config: { basePath: string }): EnvironmentProviders {
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

// Derived from httpResource itself so the template compiles on every supported
// Angular version: ResourceParamsContext (chain) on 22+, unknown on 20/21 where
// the callback is typed as zero-arg.
export type ResourceRequestContext = Parameters<typeof httpResource>[0] extends (
  context: infer Context,
) => unknown
  ? Context
  : unknown;

type ReactiveRequest<Request> = (context: ResourceRequestContext) => Request | undefined;

export interface Observing<Request, Response> {
  observable(
    request: Request,
    options: ObservableOptions & { observe: 'response' },
  ): Observable<HttpResponse<Response>>;
  observable(
    request: Request,
    options: ObservableOptions & { observe: 'events' },
  ): Observable<HttpEvent<Response>>;
  observable(request: Request, options?: ObservableOptions): Observable<Response>;
}

export interface ResourcefulValue<Request, Response> {
  resource(
    reactiveReq: ReactiveRequest<Request>,
    options: BaseHttpResourceOptionsWithDefault<Response>,
  ): HttpResourceRef<Response>;
  resource<TResult>(
    reactiveReq: ReactiveRequest<Request>,
    options: BaseHttpResourceOptionsWithDefaultAndParse<TResult, Response>,
  ): HttpResourceRef<TResult>;
  resource(
    reactiveReq: ReactiveRequest<Request>,
    options?: HttpResourceOptionsUnion<Response, Response>,
  ): HttpResourceRef<Response | undefined>;
  resource<TResult>(
    reactiveReq: ReactiveRequest<Request>,
    options: BaseHttpResourceOptionsWithParse<TResult, Response>,
  ): HttpResourceRef<TResult | undefined>;
}

export interface ResourcefulVoid<Request> {
  resource(
    reactiveReq: ReactiveRequest<Request>,
    options?: HttpResourceOptionsUnion<void, unknown>,
  ): HttpResourceRef<void>;
}

// The `.resource()` surface shared by the standalone and the bound form;
// `validateRest` accepts either through it.
export type Resourceful<Request, Response> = [Response] extends [void]
  ? ResourcefulVoid<Request>
  : ResourcefulValue<Request, Response>;

export interface RequestFnValue<Request, Response>
  extends Observing<Request, Response>, ResourcefulValue<Request, Response> {
  request(request: Request): CommonRequest;
}

export interface RequestFnVoid<Request>
  extends Observing<Request, void>, ResourcefulVoid<Request> {
  request(request: Request): CommonRequest;
}

// DI-bound form: HttpClient and base path were resolved once from an injector.
export type RequestFn<Request, Response> = [Response] extends [void]
  ? RequestFnVoid<Request>
  : RequestFnValue<Request, Response>;

export interface OperationValue<Request, Response>
  extends Observing<Request, Response>, ResourcefulValue<Request, Response> {
  // Relative URL unless `injector` is given; never calls `inject()` itself.
  request(request: Request, options?: OperationRequestOptions): CommonRequest;
  withInjector(injector?: Injector): RequestFnValue<Request, Response>;
}

export interface OperationVoid<Request>
  extends Observing<Request, void>, ResourcefulVoid<Request> {
  request(request: Request, options?: OperationRequestOptions): CommonRequest;
  withInjector(injector?: Injector): RequestFnVoid<Request>;
}

// Standalone form: `.observable()` / `.resource()` resolve DI per call from
// `options.injector` or the current injection context.
export type Operation<Request, Response> = [Response] extends [void]
  ? OperationVoid<Request>
  : OperationValue<Request, Response>;

export interface ZeroArgObserving<Response> {
  observable(
    options: ObservableOptions & { observe: 'response' },
  ): Observable<HttpResponse<Response>>;
  observable(
    options: ObservableOptions & { observe: 'events' },
  ): Observable<HttpEvent<Response>>;
  observable(options?: ObservableOptions): Observable<Response>;
}

export interface ZeroArgResourcefulValue<Response> {
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

export interface ZeroArgResourcefulVoid {
  resource(options?: HttpResourceOptionsUnion<void, unknown>): HttpResourceRef<void>;
}

export interface ZeroArgRequestFnValue<Response>
  extends ZeroArgObserving<Response>, ZeroArgResourcefulValue<Response> {
  request(): CommonRequest;
}

export interface ZeroArgRequestFnVoid
  extends ZeroArgObserving<void>, ZeroArgResourcefulVoid {
  request(): CommonRequest;
}

export type ZeroArgRequestFn<Response> = [Response] extends [void]
  ? ZeroArgRequestFnVoid
  : ZeroArgRequestFnValue<Response>;

export interface ZeroArgOperationValue<Response>
  extends ZeroArgObserving<Response>, ZeroArgResourcefulValue<Response> {
  request(options?: OperationRequestOptions): CommonRequest;
  withInjector(injector?: Injector): ZeroArgRequestFnValue<Response>;
}

export interface ZeroArgOperationVoid
  extends ZeroArgObserving<void>, ZeroArgResourcefulVoid {
  request(options?: OperationRequestOptions): CommonRequest;
  withInjector(injector?: Injector): ZeroArgRequestFnVoid;
}

export type ZeroArgOperation<Response> = [Response] extends [void]
  ? ZeroArgOperationVoid
  : ZeroArgOperationValue<Response>;

type Builder<Request> = (request: Request) => CommonRequest;

export interface DefineOperation {
  <Request, Response>(
    name: string,
    builder: Builder<Request>,
  ): Operation<Request, Response>;
  blob<Request>(name: string, builder: Builder<Request>): Operation<Request, Blob>;
  text<Request>(name: string, builder: Builder<Request>): Operation<Request, string>;
  arrayBuffer<Request>(
    name: string,
    builder: Builder<Request>,
  ): Operation<Request, ArrayBuffer>;
  zeroArg: {
    <Response>(name: string, builder: () => CommonRequest): ZeroArgOperation<Response>;
    blob(name: string, builder: () => CommonRequest): ZeroArgOperation<Blob>;
    text(name: string, builder: () => CommonRequest): ZeroArgOperation<string>;
    arrayBuffer(
      name: string,
      builder: () => CommonRequest,
    ): ZeroArgOperation<ArrayBuffer>;
  };
}

export interface RequestFactory {
  <Request, Response>(builder: Builder<Request>): RequestFn<Request, Response>;
  blob<Request>(builder: Builder<Request>): RequestFn<Request, Blob>;
  text<Request>(builder: Builder<Request>): RequestFn<Request, string>;
  arrayBuffer<Request>(builder: Builder<Request>): RequestFn<Request, ArrayBuffer>;
  zeroArg: {
    <Response>(builder: () => CommonRequest): ZeroArgRequestFn<Response>;
    blob(builder: () => CommonRequest): ZeroArgRequestFn<Blob>;
    text(builder: () => CommonRequest): ZeroArgRequestFn<string>;
    arrayBuffer(builder: () => CommonRequest): ZeroArgRequestFn<ArrayBuffer>;
  };
}

// Maps each operation in a record to its bound form.
export type Bound<T> = T extends { withInjector(injector?: Injector): infer B }
  ? B
  : never;

type Bindable = { withInjector(injector?: Injector): unknown };

export function withInjector<R extends Record<string, Bindable>>(
  record: R,
  injector?: Injector,
): { [K in keyof R]: Bound<R[K]> } {
  const resolved = resolveInjector('withInjector()', injector);
  const bound: Record<string, unknown> = {};
  for (const [key, operation] of Object.entries(record)) {
    bound[key] = operation.withInjector(resolved);
  }
  return bound as { [K in keyof R]: Bound<R[K]> };
}

function joinBasePath(base: string, url: string): string {
  // Absolute URLs (https://…, etc.) bypass the configured basePath.
  if (/^[a-z][a-z0-9+.-]*:\/\//i.test(url)) return url;
  const normalizedBase = base.endsWith('/') ? base.slice(0, -1) : base;
  const normalizedUrl = url.startsWith('/') ? url : `/${url}`;
  return normalizedBase + normalizedUrl;
}

function resolveInjector(callSite: string, injector: Injector | undefined): Injector {
  if (injector) return injector;
  // Dev builds only: the CLI defines ngDevMode=false when optimizing, so this
  // block and its string are stripped. Prod falls through to Angular's NG0203.
  if (typeof ngDevMode === 'undefined' || ngDevMode) {
    try {
      return inject(Injector);
    } catch (cause) {
      throw new Error(
        `openapi-ng: ${callSite} was called outside an injection context. ` +
          'Call it from a field initializer or constructor, pass { injector }, ' +
          'or bind once with withInjector().',
        { cause },
      );
    }
  }
  return inject(Injector);
}

interface BoundContext {
  http: HttpClient;
  basePath: string | null;
  injector: Injector;
}

function contextFrom(injector: Injector): BoundContext {
  return {
    http: injector.get(HttpClient),
    basePath: injector.get(OPENAPI_NG_BASE_PATH, null),
    injector,
  };
}

type ResourceImpl = (
  // Optional: on Angular 20/21 httpResource expects a zero-arg callback and
  // invokes it with no context.
  request: (context?: ResourceRequestContext) => CommonRequest | undefined,
  options?: HttpResourceOptions<unknown, unknown>,
) => HttpResourceRef<unknown>;

// Response decoding per factory variant: HttpClient responseType plus the
// matching httpResource flavour.
type Kind = [ResponseType | undefined, ResourceImpl];

const JSON_KIND: Kind = [undefined, (request, options) => httpResource(request, options)];
const BLOB_KIND: Kind = [
  'blob',
  (request, options) =>
    httpResource.blob(request, options as HttpResourceOptions<Blob, Blob>),
];
const TEXT_KIND: Kind = [
  'text',
  (request, options) =>
    httpResource.text(request, options as HttpResourceOptions<string, string>),
];
const ARRAY_BUFFER_KIND: Kind = [
  'arraybuffer',
  (request, options) =>
    httpResource.arrayBuffer(
      request,
      options as HttpResourceOptions<ArrayBuffer, ArrayBuffer>,
    ),
];

// One class for both forms and both arities. `bound` set means DI was
// resolved once at bind time; `zeroArg` shifts the argument positions. Types
// are applied at the `defineOperation` / `requestFactory` boundary.
class OperationImpl {
  constructor(
    private readonly name: string,
    private readonly zeroArg: boolean,
    private readonly kind: Kind,
    private readonly builder: Builder<unknown>,
    private readonly bound: BoundContext | undefined,
  ) {}

  private args<First, Options>(
    first: unknown,
    second: unknown,
  ): [First | undefined, Options | undefined] {
    return this.zeroArg
      ? [undefined, first as Options | undefined]
      : [first as First, second as Options | undefined];
  }

  private build(request: unknown, basePath: string | null): CommonRequest {
    const common = this.builder(request);
    return basePath ? { ...common, url: joinBasePath(basePath, common.url) } : common;
  }

  request(first?: unknown, second?: unknown): CommonRequest {
    const [request, options] = this.args<unknown, OperationRequestOptions>(first, second);
    const basePath = this.bound
      ? this.bound.basePath
      : (options?.injector?.get(OPENAPI_NG_BASE_PATH, null) ?? null);
    return this.build(request, basePath);
  }

  observable(first?: unknown, second?: unknown): Observable<unknown> {
    const [request, options] = this.args<unknown, ObservableOptions>(first, second);
    const { injector, ...rest }: ObservableOptions = options ?? {};
    const context =
      this.bound ?? contextFrom(resolveInjector(`${this.name}.observable()`, injector));
    const common = this.build(request, context.basePath);
    const responseType = this.kind[0];
    const merged = {
      ...rest,
      body: common.body,
      headers: common.headers,
      params: common.params,
      ...(responseType ? { responseType } : {}),
    };
    return context.http.request(common.method, common.url, merged);
  }

  resource(first?: unknown, second?: unknown): HttpResourceRef<unknown> {
    const [reactiveReq, options] = this.args<
      ReactiveRequest<unknown>,
      HttpResourceOptionsUnion<unknown, unknown>
    >(first, second);
    const injector =
      options?.injector ??
      this.bound?.injector ??
      resolveInjector(`${this.name}.resource()`, undefined);
    // Resolved here, not inside the reactive callback: that callback is not an
    // injection context.
    const basePath = this.bound
      ? this.bound.basePath
      : injector.get(OPENAPI_NG_BASE_PATH, null);
    return this.kind[1](
      context => {
        if (!reactiveReq) return this.build(undefined, basePath);
        const request = reactiveReq(context as ResourceRequestContext);
        return request === undefined ? undefined : this.build(request, basePath);
      },
      { ...options, injector },
    );
  }

  bind(callSite: string, injector: Injector | undefined): OperationImpl {
    return new OperationImpl(
      this.name,
      this.zeroArg,
      this.kind,
      this.builder,
      contextFrom(resolveInjector(callSite, injector)),
    );
  }

  withInjector(injector?: Injector): OperationImpl {
    return this.bind(`${this.name}.withInjector()`, injector);
  }
}

type Make = (zeroArg: boolean, kind: Kind) => object;

function family<T>(make: Make): T {
  return Object.assign(make(false, JSON_KIND), {
    blob: make(false, BLOB_KIND),
    text: make(false, TEXT_KIND),
    arrayBuffer: make(false, ARRAY_BUFFER_KIND),
    zeroArg: Object.assign(make(true, JSON_KIND), {
      blob: make(true, BLOB_KIND),
      text: make(true, TEXT_KIND),
      arrayBuffer: make(true, ARRAY_BUFFER_KIND),
    }),
  }) as unknown as T;
}

export const defineOperation = family<DefineOperation>(
  (zeroArg, kind) => (name: string, builder: Builder<unknown>) =>
    new OperationImpl(name, zeroArg, kind, builder, undefined),
);

// Sugar over `defineOperation(...).withInjector()`: resolves DI eagerly, so it
// still throws at construction when used outside an injection context.
export const requestFactory = family<RequestFactory>(
  (zeroArg, kind) => (builder: Builder<unknown>) =>
    new OperationImpl('requestFactory', zeroArg, kind, builder, undefined).bind(
      'requestFactory()',
      undefined,
    ),
);
