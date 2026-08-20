// This file is INTENDED TO FAIL TypeScript compilation.
// It exists so the test suite catches type-soundness regressions on the
// `debounce` option, which `RestValidatorOptions` inherits from Angular's own
// `AsyncValidatorOptions` via `Omit`. That indirection is what keeps the emitted
// template compiling on @angular/forms 21 (where the key is absent), but it
// would also silently swallow a bad value if the inherited type ever widened to
// `any`. A string is not a `DebounceTimer`, so tsc must reject it.
//
// Expected error: TS2322 — `string` is not assignable to
// `DebounceTimer<UpdatePetParams | undefined>` (i.e. `number` or a function).
import { schema } from '@angular/forms/signals';
import type { PetRest } from '../../generated/rest/pet.rest.generated';
import { validateRest } from '../../generated/rest.validate';

declare const service: PetRest;

export const shouldFail = schema<string>(path => {
  validateRest(path, service.updatePet, {
    request: ctx => ({
      petId: ctx.value(),
      body: { status: 'available' as const, tagIds: [] },
    }),
    // `debounce` must be a duration in milliseconds or a debouncer function;
    // 'soon' is neither. tsc must reject this option-bag.
    debounce: 'soon',
    onError: () => ({ kind: 'never' as const }),
  });
});
