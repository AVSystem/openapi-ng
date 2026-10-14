// Must not compile: `onSuccess`'s `result` stays typed as the endpoint's
// response.
import { schema } from '@angular/forms/signals';
import type { PetRest, GetPetParams } from '../../generated/rest/pet.rest';
import type { Pet } from '../../generated/model.ts';
import { validateRest } from '../../generated/rest.validate';

declare const service: PetRest;

export const shouldFail = schema<string>(path => {
  validateRest<GetPetParams, Pet, string>(path, service.getPet, {
    request: ctx => ({
      petId: ctx.value(),
    }),
    onSuccess: result => {
      // `result` is typed Pet — `nonExistentField` is not on Pet. tsc must
      // reject this access.
      return result.nonExistentField === 'x' ? { kind: 'never' as const } : undefined;
    },
    onError: () => ({ kind: 'never' as const }),
  });
});
