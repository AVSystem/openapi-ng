import { defineOperation } from '../../rest.util';
import type { Pet } from '../../model';

export const previewDeletePet = /* @__PURE__ */ defineOperation<PreviewDeletePetParams, Pet>(
  'previewDeletePet',
  (request: PreviewDeletePetParams) => {
    const { petId } = request;
    return {
      method: 'DELETE',
      url: `/pets/${encodeURIComponent(petId)}/preview-delete`,
    };
  },
);

export interface PreviewDeletePetParams {
  petId: string;
}
