import { defineMutation } from '../../rest.util';

export const exportPet = /* @__PURE__ */ defineMutation.arrayBuffer<ExportPetParams>(
  'exportPet',
  (request: ExportPetParams) => {
    const { petId } = request;
    return {
      method: 'POST',
      url: `/pets/${encodeURIComponent(petId)}/export`,
    };
  },
);

export interface ExportPetParams {
  petId: string;
}
