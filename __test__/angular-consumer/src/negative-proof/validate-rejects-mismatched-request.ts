// This file is INTENDED TO FAIL TypeScript compilation.
// It exists so the test suite catches type-soundness regressions on the
// validateRest surface: the `request` callback's return type MUST stay
// pinned to the endpoint's Request shape (here, UpdatePetParams), never
// widen to `any`/`unknown`. If the typing widened, the
// `{ wrong: 'value' }` literal below would be accepted and tsc would
// exit 0 — causing the negative-compile test to fail and alerting us.
//
// Expected error: TS2322 — `{ wrong: string }` is not assignable to
// `UpdatePetParams | undefined`.
//
// Note: we let `TRequest` be inferred from `service.updatePet` (which
// pins it to `UpdatePetParams`) rather than supplying it explicitly,
// so the type conflict surfaces as a TS2322 assignability error on the
// `request` property of the option-bag — exactly the surface this
// proof is meant to lock down — instead of a TS2345 argument-type
// error on the `service.updatePet` position.
import { schema } from '@angular/forms/signals';
import type { PetRest } from '../../generated/rest/pet.rest.generated';
import { validateRest } from '../../generated/rest.validate';

declare const service: PetRest;

// The mismatched value is annotated with an unrelated interface so the
// failure becomes an unambiguous TS2322 assignability error (named-type
// vs named-type) rather than the more specialised TS2739 "missing
// properties from object literal" diagnostic.
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
