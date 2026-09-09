// Must not compile: `validateRest`'s `request` callback stays pinned to
// the endpoint's request shape.
import { schema } from '@angular/forms/signals';
import type { PetRest } from '../../generated/rest/pet.rest.generated';
import { validateRest } from '../../generated/rest.validate';

declare const service: PetRest;

interface WrongRequest {
  wrong: string;
}
declare const wrongRequest: WrongRequest;

export const shouldFail = schema<string>(path => {
  validateRest(path, service.updatePet, {
    // `request` must return UpdatePetParams | undefined; a value of
    // type `WrongRequest` does not satisfy it. tsc must reject this
    // option-bag with TS2322.
    request: () => wrongRequest,
    onError: () => ({ kind: 'never' as const }),
  });
});
