// Type-proof for the `both` layout: standalone operations, the bound form,
// the record helper, the barrel namespace and the aliased reserved name.
// Same declare-/expectType-based style as service-proof.ts: tsc --noEmit
// gate only, no runtime. Generated from reserved-method-name.openapi.yaml.

import type { HttpResourceRef } from '@angular/common/http';
import { Injector, inject } from '@angular/core';
import { schema } from '@angular/forms/signals';
import type { Observable } from 'rxjs';
import type { Pet, PetList, Problem } from '../generated/model';
import type { CommonRequest } from '../generated/rest.model';
import { withInjector, type Operation, type RequestFn } from '../generated/rest.util';
import { validateRest } from '../generated/rest.validate';
import * as ops from '../generated/rest/pet';
import type { GetPetError, GetPetParams, PetRest } from '../generated/rest/pet.rest';
import { delete as deletePet, type DeleteParams } from '../generated/rest/pet/delete';
import { getPet } from '../generated/rest/pet/get-pet';
import { listPets, type ListPetsParams } from '../generated/rest/pet/list-pets';

declare function expectType<T>(value: T): void;
declare const injector: Injector;
declare const service: PetRest;

// Standalone form inside an injection context (field initialiser) and
// outside one (handler with an explicit injector).
class PetsComponent {
  readonly pets = listPets.resource(() => ({ status: 'available' }), {
    defaultValue: [],
  });
  readonly #injector = inject(Injector);

  remove(petId: string) {
    return deletePet.observable({ petId }, { injector: this.#injector });
  }
}

declare const component: PetsComponent;
expectType<HttpResourceRef<PetList>>(component.pets);
expectType<Observable<void>>(component.remove('x'));

// `.request()` is pure without options and base-pathed with an injector;
// both return the same descriptor type.
expectType<CommonRequest>(listPets.request({}));
expectType<CommonRequest>(listPets.request({ status: 'sold' }, { injector }));

expectType<Operation<ListPetsParams, PetList>>(listPets);
expectType<Operation<DeleteParams, void>>(deletePet);

// Bound form: today's RequestFn, identical to the `services` class property.
const boundGetPet = getPet.withInjector(injector);
expectType<RequestFn<GetPetParams, Pet>>(boundGetPet);
expectType<RequestFn<GetPetParams, Pet>>(service.getPet);
expectType<Observable<Pet>>(boundGetPet.observable({ petId: 'x' }));
expectType<HttpResourceRef<Pet | undefined>>(
  boundGetPet.resource(() => ({ petId: 'x' })),
);

// Record helper: every entry maps to its RequestFn.
const api = withInjector({ getPet, deletePet, listPets }, injector);
expectType<RequestFn<GetPetParams, Pet>>(api.getPet);
expectType<RequestFn<DeleteParams, void>>(api.deletePet);
expectType<RequestFn<ListPetsParams, PetList>>(api.listPets);
expectType<Observable<void>>(api.deletePet.observable({ petId: 'x' }));

// Barrel namespace, reserved-word member included.
expectType<Operation<DeleteParams, void>>(ops.delete);
expectType<Operation<GetPetParams, Pet>>(ops.getPet);
expectType<CommonRequest>(ops.listPets.request({}));

// The class file re-exports the per-operation interfaces.
declare const notFound: GetPetError;
expectType<Problem>(notFound[404]);

// validateRest accepts a standalone operation and a bound one.
schema<string>(path => {
  validateRest<GetPetParams, Pet, string>(path, getPet, {
    request: ctx => ({ petId: ctx.value() }),
    onError: () => ({ kind: 'validation-unavailable' as const }),
  });
  validateRest<GetPetParams, Pet, string>(path, service.getPet, {
    request: ctx => ({ petId: ctx.value() }),
    onError: () => ({ kind: 'validation-unavailable' as const }),
  });
});

// @ts-expect-error — a requestful operation needs its request argument
listPets.observable();

// @ts-expect-error — the bound form does not expose withInjector
boundGetPet.withInjector(injector);

// @ts-expect-error — nor does any entry of a bound record
api.getPet.withInjector(injector);

// @ts-expect-error — the bound `.request()` takes no options
service.listPets.request({}, { injector });
