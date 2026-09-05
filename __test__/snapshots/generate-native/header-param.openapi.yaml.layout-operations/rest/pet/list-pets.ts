import { defineOperation } from '../../rest.util';

export const listPets = defineOperation<ListPetsParams, void>(
  'listPets',
  (request: ListPetsParams) => {
    const { headers } = request;
    return {
      method: 'GET',
      url: `/pets`,
      headers,
    };
  },
);

export interface ListPetsParams {
  headers: {
    'X-Api-Key': string;
  };
}
