import { defineOperation, httpParams } from '../../rest.util';
import type { PetList } from '../../model';

/**
 * List pets, optionally filtered by status.
 */
export const listPets = /* @__PURE__ */ defineOperation<ListPetsParams, PetList>(
  'listPets',
  (request: ListPetsParams) => {
    const { status } = request;
    return {
      method: 'GET',
      url: `/pets`,
      params: httpParams({ status }),
    };
  },
);

export interface ListPetsParams {
  status?: string;
}
