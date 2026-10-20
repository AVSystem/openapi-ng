import { defineMutation } from '../../rest.util';
import type { Pet } from '../../model';

export const createPet = /* @__PURE__ */ defineMutation<CreatePetParams, Pet>(
  'createPet',
  (request: CreatePetParams) => {
    const { body } = request;
    return {
      method: 'POST',
      url: `/pets`,
      body: body,
    };
  },
);

export interface CreatePetParams {
  body: Pet;
}
