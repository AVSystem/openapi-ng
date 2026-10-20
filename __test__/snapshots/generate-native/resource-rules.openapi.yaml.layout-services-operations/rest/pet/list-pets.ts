import { defineOperation } from '../../rest.util';
import type { Pet } from '../../model';

export const listPets = /* @__PURE__ */ defineOperation.zeroArg<Pet[]>(
  'listPets',
  () => ({
    method: 'GET',
    url: `/pets`,
  }),
);
