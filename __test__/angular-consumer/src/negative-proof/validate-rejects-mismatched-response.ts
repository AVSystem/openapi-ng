// This file is INTENDED TO FAIL TypeScript compilation.
// It exists so the test suite catches type-soundness regressions on the
// validateRest surface: the `onSuccess` callback's `result` parameter
// MUST be typed as the endpoint's Response (here, Pet), never widen to
// `any`/`unknown`. If `result` were widened, the `result.nonExistentField`
// access below would be accepted and tsc would exit 0 — causing the
// negative-compile test to fail and alerting us.
//
// Expected error: TS2339 — property 'nonExistentField' does not exist on
// type 'Pet'.
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
