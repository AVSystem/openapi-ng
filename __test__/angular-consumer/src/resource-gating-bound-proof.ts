// Compile-time proofs for `layout: ['services', 'operations']` with `application/pdf` as
// `arrayBuffer`: a bound class keeps each mutation's type, so it has no `.resource()`.

import type { HttpResourceRef } from '@angular/common/http';
import type { Observable } from 'rxjs';

import type { Pet } from '../generated/model';
import * as ops from '../generated/rest/pet';
import type { PetRest } from '../generated/rest/pet.rest';
import type { SessionRest } from '../generated/rest/session.rest';

declare const service: PetRest;
declare const session: SessionRest;
declare function expectType<T>(value: T): void;

expectType<HttpResourceRef<Pet[] | undefined>>(service.listPets.resource());
expectType<Observable<ArrayBuffer>>(service.exportPet.observable({ petId: 'p-1' }));
expectType<Observable<void>>(session.logout.observable());

// @ts-expect-error — an `arrayBuffer` write has no `.resource()`
service.exportPet.resource(() => ({ petId: 'p-1' }));

// @ts-expect-error — nor has the operation it is bound from
ops.exportPet.resource(() => ({ petId: 'p-1' }));

// @ts-expect-error — nor has a zero-arg GET opted out by the extension
session.logout.resource();
