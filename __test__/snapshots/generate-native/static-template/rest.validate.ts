import type { Signal } from '@angular/core';
import { validateAsync } from '@angular/forms/signals';
import type {
  FieldContext,
  MapToErrorsFn,
  PathKind,
  SchemaPath,
  SchemaPathRules,
  TreeValidationResult,
} from '@angular/forms/signals';
import type { BaseHttpResourceOptions } from './rest.model';
import type { RequestFn, RequestFnValue } from './rest.util';

export type { MapToErrorsFn } from '@angular/forms/signals';

export interface RestValidatorOptions<
  TRequest,
  TResponse,
  TValue,
  TPathKind extends PathKind = PathKind.Root,
> {
  request: (ctx: FieldContext<TValue, TPathKind>) => TRequest | undefined;
  onError: (error: unknown, ctx: FieldContext<TValue, TPathKind>) => TreeValidationResult;
  onSuccess?: MapToErrorsFn<TValue, TResponse, TPathKind>;
  options?: BaseHttpResourceOptions<TResponse, TResponse>;
}

export function validateRest<
  TRequest,
  TResponse,
  TValue,
  TPathKind extends PathKind = PathKind.Root,
>(
  path: SchemaPath<TValue, SchemaPathRules.Supported, TPathKind>,
  requestFn: RequestFn<TRequest, TResponse>,
  opts: RestValidatorOptions<TRequest, TResponse, TValue, TPathKind>,
): void {
  validateAsync<TValue, TRequest | undefined, TResponse | undefined, TPathKind>(path, {
    params: opts.request,
    factory: (req: Signal<TRequest | undefined>) =>
      (requestFn as RequestFnValue<TRequest, TResponse>).resource(req, opts.options),
    onSuccess: (result, ctx) =>
      result === undefined ? undefined : (opts.onSuccess ?? (() => undefined))(result, ctx),
    onError: opts.onError,
  });
}
