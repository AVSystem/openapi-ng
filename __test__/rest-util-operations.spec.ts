// Runtime behaviour of the standalone-operation surface in
// `templates/angular/rest.util.ts`, run against the template source
// itself. `@angular/common/http` is partially compiled, so the JIT
// compiler must be loaded first; `Injector.create` and
// `runInInjectionContext` need no platform or DOM. `.resource()` is not
// exercised here — `httpResource` needs an environment injector — its
// wiring is covered by the angular-consumer type proofs.
import '@angular/compiler';
import test from 'ava';
import { Injector, runInInjectionContext } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import {
  OPENAPI_NG_BASE_PATH,
  defineOperation,
  requestFactory,
  withInjector,
} from '../templates/angular/rest.util';

interface RecordedCall {
  method: string;
  url: string;
  options: Record<string, unknown>;
}

function fakeHttp(calls: RecordedCall[]) {
  return {
    request: (method: string, url: string, options: Record<string, unknown>) => {
      calls.push({ method, url, options });
      return 'observable';
    },
  };
}

function injectorWith(basePath: string | null, calls: RecordedCall[], parent?: Injector) {
  return Injector.create({
    parent,
    providers: [
      { provide: HttpClient, useValue: fakeHttp(calls) },
      ...(basePath === null
        ? []
        : [{ provide: OPENAPI_NG_BASE_PATH, useValue: basePath }]),
    ],
  });
}

interface ListPetsParams {
  status?: string;
}

const listPets = defineOperation<ListPetsParams, unknown>('listPets', ({ status }) => ({
  method: 'GET',
  url: '/pets',
  params: status === undefined ? undefined : { status },
}));

const ping = defineOperation.zeroArg.blob('ping', () => ({ method: 'GET', url: 'ping' }));

test('standalone .observable() outside an injection context throws with NG0203 as cause', t => {
  const err = t.throws(() => listPets.observable({}));
  t.true(
    err!.message.startsWith(
      'openapi-ng: listPets.observable() was called outside an injection context.',
    ),
  );
  t.true(err!.message.includes('pass { injector }'));
  t.true(err!.message.includes('withInjector()'));
  t.is((err!.cause as { code?: number }).code, -203);
});

test('standalone .observable() with { injector } issues the request through that injector', t => {
  const calls: RecordedCall[] = [];
  const injector = injectorWith('/api', calls);

  const result = listPets.observable(
    { status: 'sold' },
    { injector, observe: 'response' },
  );

  t.is(result, 'observable' as never);
  t.deepEqual(calls, [
    {
      method: 'GET',
      url: '/api/pets',
      options: {
        observe: 'response',
        body: undefined,
        headers: undefined,
        params: { status: 'sold' },
      },
    },
  ]);
});

test('standalone .observable() inside an injection context needs no injector', t => {
  const calls: RecordedCall[] = [];
  const injector = injectorWith('/api', calls);

  runInInjectionContext(injector, () => ping.observable());

  t.is(calls.length, 1);
  t.is(calls[0].url, '/api/ping');
  t.is(calls[0].options.responseType, 'blob');
});

test('a child injector base path overrides the parent one', t => {
  const calls: RecordedCall[] = [];
  const parent = injectorWith('/parent', calls);
  const child = Injector.create({
    parent,
    providers: [{ provide: OPENAPI_NG_BASE_PATH, useValue: '/child' }],
  });

  listPets.observable({}, { injector: child });

  t.is(calls[0].url, '/child/pets');
  t.is(listPets.request({}, { injector: child }).url, '/child/pets');
  t.is(listPets.request({}, { injector: parent }).url, '/parent/pets');
});

test('standalone .request() returns the relative URL, even inside an injection context', t => {
  const injector = injectorWith('/api', []);

  t.is(listPets.request({}).url, '/pets');
  t.is(runInInjectionContext(injector, () => listPets.request({})).url, '/pets');
  t.is(ping.request().url, 'ping');
});

test('.request() with { injector } and the bound form prepend the base path', t => {
  const injector = injectorWith('/api', []);

  t.is(listPets.request({}, { injector }).url, '/api/pets');
  t.is(listPets.withInjector(injector).request({}).url, '/api/pets');
  t.is(ping.request({ injector }).url, '/api/ping');
  t.is(ping.withInjector(injector).request().url, '/api/ping');
});

test('a bound operation without a configured base path leaves the URL untouched', t => {
  const injector = injectorWith(null, []);
  t.is(listPets.withInjector(injector).request({}).url, '/pets');
});

test('withInjector(record, injector) binds every entry to that injector', t => {
  const calls: RecordedCall[] = [];
  const injector = injectorWith('/api', calls);

  const api = withInjector({ listPets, ping }, injector);
  api.listPets.observable({ status: 'available' });
  api.ping.observable();

  t.deepEqual(Object.keys(api), ['listPets', 'ping']);
  t.deepEqual(
    calls.map(call => call.url),
    ['/api/pets', '/api/ping'],
  );
  t.false('withInjector' in Object.getOwnPropertyDescriptors(api.listPets));
});

test('withInjector() without an injector outside an injection context throws', t => {
  const err = t.throws(() => listPets.withInjector());
  t.true(
    err!.message.startsWith('openapi-ng: listPets.withInjector() was called outside'),
  );
  const recordErr = t.throws(() => withInjector({ listPets }));
  t.true(recordErr!.message.startsWith('openapi-ng: withInjector() was called outside'));
});

test('requestFactory(...) outside an injection context throws at construction', t => {
  const err = t.throws(() => requestFactory(() => ({ method: 'GET', url: '/x' })));
  t.true(err!.message.startsWith('openapi-ng: requestFactory() was called outside'));
  t.is((err!.cause as { code?: number }).code, -203);
});

test('requestFactory(...) inside an injection context resolves DI once and keeps working after the injector is destroyed', t => {
  const calls: RecordedCall[] = [];
  const injector = injectorWith('/api', calls);

  const legacy = runInInjectionContext(injector, () =>
    requestFactory<ListPetsParams, unknown>(({ status }) => ({
      method: 'GET',
      url: '/legacy',
      params: status === undefined ? undefined : { status },
    })),
  );
  const bound = listPets.withInjector(injector);
  injector.destroy();

  t.throws(() => injector.get(HttpClient));
  t.is(legacy.request({}).url, '/api/legacy');
  legacy.observable({ status: 'sold' });
  bound.observable({});
  t.deepEqual(
    calls.map(call => call.url),
    ['/api/legacy', '/api/pets'],
  );
});
