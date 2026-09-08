import { defineOperation } from '../../rest.util';
import type { PetList } from '../../model';

export const listPets = /* @__PURE__ */ defineOperation.zeroArg<PetList>(
  'listPets',
  () => ({
    method: 'GET',
    url: `/pets`,
  }),
);
