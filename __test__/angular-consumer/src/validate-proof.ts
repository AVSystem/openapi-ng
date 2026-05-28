// Positive type-proof for `validateRest`. Exercises the four shapes the
// consumer can reach for: typed Request param, body-inspecting onSuccess,
// cheap "200 = valid" omission of onSuccess, and the void-response branch.
//
// We build a real `SchemaPath<string>` inside `schema((p) => { ... })` —
// that's how consumers wire signal-forms validators in practice. Mirrors
// the declare-/expectType-based style of service-proof.ts: no runtime,
// just a tsc --noEmit gate.

import { schema } from '@angular/forms/signals';
import type { PetRest, UpdatePetParams } from '../generated/rest/pet.rest.generated';
import type { Pet } from '../generated/model.generated.ts';
import type { RequestFnVoid } from '../generated/rest.util';
import { validateRest } from '../generated/rest.validate';

declare const service: PetRest;
declare function expectType<T>(value: T): void;

// Body-inspecting onSuccess: response is typed as `Pet`, request is typed
// as `UpdatePetParams`, and the validator error shape is the consumer's
// own discriminated union (here keyed by `kind`).
schema<string>(path => {
  validateRest<UpdatePetParams, Pet, string>(path, service.updatePet, {
    request: ctx => {
      const value: string = ctx.value();
      expectType<string>(value);
      return {
        petId: value,
        body: { status: 'available', tagIds: [] },
      };
    },
    onSuccess: response => {
      expectType<Pet>(response);
      return response.tags.length === 0
        ? { kind: 'pet-has-no-tags' as const }
        : undefined;
    },
    onError: () => ({ kind: 'validation-unavailable' as const }),
  });
});

// Cheap call site: `onSuccess` omitted — "HTTP 200 = valid" semantics.
// Request param is still strongly typed; we only assert reachability of
// the no-onSuccess overload.
schema<string>(path => {
  validateRest<UpdatePetParams, Pet, string>(path, service.updatePet, {
    request: ctx => ({
      petId: ctx.value(),
      body: { status: 'available', tagIds: [] },
    }),
    onError: () => undefined,
  });
});

// Void-response endpoint. petstore-rich has no 204 op, so we synthesise
// a `RequestFnVoid<{ id: string }>` against the rest.util interface —
// matches the pattern at service-proof.ts:99-100. The TResponse generic
// must be `void`; onSuccess is unreachable but the compiler still checks
// the option-bag shape.
declare const voidEndpoint: RequestFnVoid<{ id: string }>;
schema<string>(path => {
  validateRest<{ id: string }, void, string>(path, voidEndpoint, {
    request: ctx => ({ id: ctx.value() }),
    onError: () => undefined,
  });
});
