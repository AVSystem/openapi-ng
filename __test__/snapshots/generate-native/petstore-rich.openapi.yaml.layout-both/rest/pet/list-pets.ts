import { defineOperation } from '../../rest.util';
import type { PetList } from '../../model';

export const listPets = defineOperation.zeroArg<PetList>(
  'listPets',
  () => ({
    method: 'GET',
    url: `/pets`,
  }),
);
