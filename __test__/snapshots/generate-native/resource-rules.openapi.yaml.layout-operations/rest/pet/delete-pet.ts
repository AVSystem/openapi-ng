import { defineMutation } from '../../rest.util';

export const deletePet = /* @__PURE__ */ defineMutation<DeletePetParams, void>(
  'deletePet',
  (request: DeletePetParams) => {
    const { petId } = request;
    return {
      method: 'DELETE',
      url: `/pets/${encodeURIComponent(petId)}`,
    };
  },
);

export interface DeletePetParams {
  petId: string;
}
