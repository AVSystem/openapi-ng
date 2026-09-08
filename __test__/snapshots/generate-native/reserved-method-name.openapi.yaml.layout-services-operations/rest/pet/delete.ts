import { defineOperation } from '../../rest.util';

/**
 * Remove a pet. The operationId is a reserved word on purpose.
 */
const delete_ = /* @__PURE__ */ defineOperation<DeleteParams, void>(
  'delete',
  (request: DeleteParams) => {
    const { petId } = request;
    return {
      method: 'DELETE',
      url: `/pets/${encodeURIComponent(petId)}`,
    };
  },
);

export { delete_ as delete };

export interface DeleteParams {
  petId: string;
}
