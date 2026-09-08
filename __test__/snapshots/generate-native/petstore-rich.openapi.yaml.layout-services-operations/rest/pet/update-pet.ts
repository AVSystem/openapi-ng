import { defineOperation, httpParams } from '../../rest.util';
import type { Pet, PetId, UpdatePetRequest } from '../../model';

export const updatePet = /* @__PURE__ */ defineOperation<UpdatePetParams, Pet>(
  'updatePet',
  (request: UpdatePetParams) => {
    const { petId, includeHistory, body } = request;
    return {
      method: 'POST',
      url: `/pets/${encodeURIComponent(petId)}`,
      params: httpParams({ includeHistory }),
      body: body,
    };
  },
);

export interface UpdatePetParams {
  petId: PetId;
  includeHistory?: boolean;
  body: UpdatePetRequest;
}
