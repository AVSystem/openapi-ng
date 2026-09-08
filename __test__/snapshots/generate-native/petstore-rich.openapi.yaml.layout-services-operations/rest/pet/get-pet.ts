import { defineOperation } from '../../rest.util';
import type { Pet, PetId } from '../../model';

export const getPet = /* @__PURE__ */ defineOperation<GetPetParams, Pet>(
  'getPet',
  (request: GetPetParams) => {
    const { petId } = request;
    return {
      method: 'GET',
      url: `/pets/${encodeURIComponent(petId)}`,
    };
  },
);

export interface GetPetParams {
  petId: PetId;
}
