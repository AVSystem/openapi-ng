// Must not compile: a multipart binary field stays `Blob | File`.
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
