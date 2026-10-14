import { defineOperation } from '../../rest.util';
import type { Pet } from '../../model';

export const searchPets = /* @__PURE__ */ defineOperation<SearchPetsParams, Pet[]>(
  'searchPets',
  (request: SearchPetsParams) => {
    const { name } = request;
    return {
      method: 'POST',
      url: `/pets/search`,
      body: { name },
    };
  },
);

export interface SearchPetsParams {
  name?: string;
}
