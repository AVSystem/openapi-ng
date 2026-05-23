// This file is INTENDED TO FAIL TypeScript compilation.
// It exists so the test suite catches type-soundness regressions on the
// multipart form-body surface: the binary field's request-interface
// type MUST stay `Blob | File`, never widen to `string`/`any`. If a
// future change accidentally collapses the binary field type, this
// assignment would succeed and tsc would exit 0 — causing the
// negative-compile test to fail and alerting us.
//
// Expected error: TS2322 — `'string-not-blob'` (a literal string) is not
// assignable to `Blob | File`.
import type { UpdatePetAvatarParams } from '../../generated/rest/pet.rest.generated';

// Construct an UpdatePetAvatarParams whose `avatar` field is a string,
// not a Blob/File. Every other field carries a valid value so the
// failure is unambiguously about the binary slot.
export const shouldFail: UpdatePetAvatarParams = {
  petId: 'p-1',
  status: 'available',
  tagIds: [],
  avatar: 'string-not-blob',
  galleries: [],
};
