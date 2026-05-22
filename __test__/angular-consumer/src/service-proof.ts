import type { PetRest, UpdatePetParams } from '../generated/rest/pet.rest.generated';
import type {
  HttpEvent,
  HttpResourceRef,
  HttpResponse,
} from '@angular/common/http';
import { Pet, PetList } from '../generated/model.generated.ts';
import { Observable } from 'rxjs';
import type {
  RequestFnVoid,
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
  reportProgress: true,
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

const updatePetObservableResponse = service.updatePet.observable(request, {
  observe: 'response',
});
const updatePetObservableEvents = service.updatePet.observable(request, {
  observe: 'events',
  reportProgress: true,
});

expectType<string>(updatePetRequest.url);
expectType<Observable<Pet>>(updatePetObservable);
expectType<HttpResourceRef<Pet | undefined>>(updatePetResource);
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
    { observe: 'events', reportProgress: true },
  ),
);
