// Must not compile: `debounce` keeps the `DebounceTimer` type it inherits
// from Angular's `AsyncValidatorOptions`.
import { schema } from '@angular/forms/signals';
import type { PetRest } from '../../generated/rest/pet.rest';
import { validateRest } from '../../generated/rest.validate';

declare const service: PetRest;

export const shouldFail = schema<string>(path => {
  validateRest(path, service.getPet, {
    request: ctx => ({
      petId: ctx.value(),
    }),
    // `debounce` must be a duration in milliseconds or a debouncer function;
    // 'soon' is neither. tsc must reject this option-bag.
    debounce: 'soon',
    onError: () => ({ kind: 'never' as const }),
  });
});
