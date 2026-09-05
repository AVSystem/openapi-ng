import { defineOperation, httpParams } from '../../rest.util';
import type { PetList } from '../../model.generated';

/**
 * List pets, optionally filtered by status.
 */
export const listPets = defineOperation<ListPetsParams, PetList>(
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
