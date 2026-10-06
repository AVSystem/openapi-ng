import type { PetRest, UpdatePetParams } from '../generated/rest/pet.rest';
import type {
  HttpClient,
  HttpEvent,
  HttpResourceRef,
  HttpResponse,
} from '@angular/common/http';
import { Pet, PetList } from '../generated/model.ts';
import type { ObservableOptions } from '../generated/rest.model';
import { Observable } from 'rxjs';
import type { ResourceParamsContext } from '@angular/core';
import type {
  RequestFnVoid,
  ResourceRequestContext,
  ZeroArgRequestFnVoid,
} from '../generated/rest.util';

declare const service: PetRest;

declare function expectType<T>(value: T): void;

/**
 * listPets
 * */

const listPetsRequest = service.listPets.request();
const listPetsObservable = service.listPets.observable();
const listPetsResource = service.listPets.resource();

const listPetsResourceDefaultValue = service.listPets.resource({
  defaultValue: [],
});
const listPetsResourceParse = service.listPets.resource<number>({
  parse: raw => raw.length,
});
const listPetsResourceParseDefaultValue = service.listPets.resource<number>({
  parse: raw => raw.length,
  defaultValue: 42,
});

const listPetsObservableResponse = service.listPets.observable({
  observe: 'response',
});
const listPetsObservableEvents = service.listPets.observable({
  observe: 'events',
  reportDownloadProgress: true,
});

expectType<string>(listPetsRequest.url);
expectType<HttpResourceRef<PetList | undefined>>(listPetsResource);
expectType<Observable<PetList>>(listPetsObservable);
expectType<HttpResourceRef<PetList>>(listPetsResourceDefaultValue);
expectType<HttpResourceRef<number | undefined>>(listPetsResourceParse);
expectType<HttpResourceRef<number>>(listPetsResourceParseDefaultValue);
expectType<Observable<HttpResponse<PetList>>>(listPetsObservableResponse);
expectType<Observable<HttpEvent<PetList>>>(listPetsObservableEvents);

/**
 * updatePet
 * */

const request: UpdatePetParams = {
  petId: 'id',
  body: {
    status: 'available',
    tagIds: [],
  },
};
const defaultPet: Pet = {
  id: 'id',
  name: 'name',
  status: 'available',
  tags: [],
};
const updatePetRequest = service.updatePet.request(request);
const updatePetObservable = service.updatePet.observable(request);
const updatePetResource = service.updatePet.resource(() => request);

const updatePetResourceDefaultValue = service.updatePet.resource(() => request, {
  defaultValue: defaultPet,
});
const updatePetResourceParse = service.updatePet.resource<number>(() => request, {
  parse: raw => raw.tags.length,
});
const updatePetResourceParseDefaultValue = service.updatePet.resource<number>(
  () => request,
  { parse: raw => raw.tags.length, defaultValue: 42 },
);

// ResourceRequestContext is derived from httpResource's callback; on Angular 22
// it must resolve to exactly ResourceParamsContext (not degrade to unknown).
declare const paramsContext: ResourceParamsContext;
declare const requestContext: ResourceRequestContext;
expectType<ResourceRequestContext>(paramsContext);
expectType<ResourceParamsContext>(requestContext);

// `chain` from ResourceParamsContext must flow through the resource() wrapper.
const updatePetResourceChained = service.updatePet.resource(context => {
  const firstPet = context.chain(listPetsResourceDefaultValue).at(0);
  return firstPet === undefined ? undefined : { ...request, petId: firstPet.id };
});

const updatePetObservableResponse = service.updatePet.observable(request, {
  observe: 'response',
});
const updatePetObservableEvents = service.updatePet.observable(request, {
  observe: 'events',
  reportDownloadProgress: true,
});

// Every HttpClient.request option the operation does not supply itself passes
// through, so options added by a later Angular need no regeneration.
type PassThroughOptions = Omit<
  NonNullable<Parameters<HttpClient['request']>[2]>,
  'body' | 'params' | 'headers' | 'responseType'
>;
// A non-literal argument skips excess-property checks, so compare keys too.
declare const missingOptionKey: Exclude<
  keyof PassThroughOptions,
  keyof ObservableOptions
>;
expectType<never>(missingOptionKey);
declare const everyRequestOption: Required<PassThroughOptions>;
service.updatePet.observable(request, everyRequestOption);
service.updatePet.observable(request, {
  integrity: 'sha256-x',
  referrerPolicy: 'no-referrer',
  reportUploadProgress: true,
});

// @ts-expect-error — the operation supplies the body
service.updatePet.observable(request, { body: {} });
// @ts-expect-error — the operation supplies the query params
service.updatePet.observable(request, { params: {} });
// @ts-expect-error — the operation supplies the headers
service.updatePet.observable(request, { headers: {} });
// @ts-expect-error — the response type is fixed per operation
service.updatePet.observable(request, { responseType: 'text' });

expectType<string>(updatePetRequest.url);
expectType<Observable<Pet>>(updatePetObservable);
expectType<HttpResourceRef<Pet | undefined>>(updatePetResource);
expectType<HttpResourceRef<Pet | undefined>>(updatePetResourceChained);
expectType<HttpResourceRef<Pet>>(updatePetResourceDefaultValue);
expectType<HttpResourceRef<number | undefined>>(updatePetResourceParse);
expectType<HttpResourceRef<number>>(updatePetResourceParseDefaultValue);
expectType<Observable<HttpResponse<Pet>>>(updatePetObservableResponse);
expectType<Observable<HttpEvent<Pet>>>(updatePetObservableEvents);

// Synthetic proofs for the void variants. petstore-rich has no 204-returning
// operations; hand-declare instances against the interfaces exported from
// rest.util so the overload set is still asserted by the tsc gate.
declare const zeroArgVoidFactory: ZeroArgRequestFnVoid;
declare const requestVoidFactory: RequestFnVoid<{ id: string }>;

expectType<Observable<void>>(zeroArgVoidFactory.observable());
expectType<Observable<HttpResponse<void>>>(
  zeroArgVoidFactory.observable({ observe: 'response' }),
);
expectType<Observable<HttpEvent<void>>>(
  zeroArgVoidFactory.observable({ observe: 'events' }),
);

expectType<Observable<void>>(requestVoidFactory.observable({ id: 'x' }));
expectType<Observable<HttpResponse<void>>>(
  requestVoidFactory.observable({ id: 'x' }, { observe: 'response' }),
);
expectType<Observable<HttpEvent<void>>>(
  requestVoidFactory.observable(
    { id: 'x' },
    { observe: 'events', reportDownloadProgress: true },
  ),
);
