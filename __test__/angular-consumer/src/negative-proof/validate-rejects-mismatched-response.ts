// Must not compile: `onSuccess`'s `result` stays typed as the endpoint's
// response.
import { schema } from '@angular/forms/signals';
import type { PetRest, UpdatePetParams } from '../../generated/rest/pet.rest.generated';
import type { Pet } from '../../generated/model.generated.ts';
import { validateRest } from '../../generated/rest.validate';

declare const service: PetRest;

export const shouldFail = schema<string>(path => {
  validateRest<UpdatePetParams, Pet, string>(path, service.updatePet, {
    request: ctx => ({
      petId: ctx.value(),
      body: { status: 'available', tagIds: [] },
    }),
    onSuccess: result => {
      // `result` is typed Pet — `nonExistentField` is not on Pet. tsc must
      // reject this access.
      return result.nonExistentField === 'x' ? { kind: 'never' as const } : undefined;
    },
    onError: () => ({ kind: 'never' as const }),
  });
});
