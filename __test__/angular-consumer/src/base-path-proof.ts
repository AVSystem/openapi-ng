import type { EnvironmentProviders, InjectionToken } from '@angular/core';
import { OPENAPI_NG_BASE_PATH, provideOpenapiNg } from '../generated/rest.util';

declare function expectType<T>(value: T): void;

expectType<InjectionToken<string>>(OPENAPI_NG_BASE_PATH);

expectType<EnvironmentProviders>(
  provideOpenapiNg({ basePath: 'https://api.example.com' }),
);

// @ts-expect-error — basePath is required
provideOpenapiNg({});

// @ts-expect-error — basePath must be a string
provideOpenapiNg({ basePath: 123 });
