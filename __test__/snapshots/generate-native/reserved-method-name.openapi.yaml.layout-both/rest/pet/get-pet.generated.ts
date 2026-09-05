import { defineOperation } from '../../rest.util';
import type { Pet, Problem } from '../../model.generated';

export const getPet = defineOperation<GetPetParams, Pet>(
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
  petId: string;
}

export interface GetPetError {
  404: Problem;
}
