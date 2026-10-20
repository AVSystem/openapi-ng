// Compile-time proofs for the `resources` rules, against the fixture
// `resource-rules.openapi.yaml`: POSTs opted in by name or extension keep
// `.resource()` and work with `validateRest`; non-JSON writes keep their kind.

import type { HttpResourceRef } from '@angular/common/http';
import { schema } from '@angular/forms/signals';
import type { Observable } from 'rxjs';

import type { Pet } from '../generated/model';
import type { PetRest } from '../generated/rest/pet.rest';
import { validateRest } from '../generated/rest.validate';

declare const service: PetRest;
declare function expectType<T>(value: T): void;

expectType<HttpResourceRef<Pet[] | undefined>>(
  service.searchPets.resource(() => ({ name: 'rex' })),
);
expectType<HttpResourceRef<Pet | undefined>>(
  service.previewDeletePet.resource(() => ({ petId: 'p-1' })),
);

schema<string>(path => {
  validateRest(path, service.searchPets, {
    request: ctx => ({ name: ctx.value() }),
    onError: () => undefined,
  });
});

expectType<Observable<Blob>>(service.exportPet.observable({ petId: 'p-1' }));
expectType<Observable<string>>(service.renderReport.observable());
