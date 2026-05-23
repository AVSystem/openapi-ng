import { Injectable } from '@angular/core';
import { requestFactory } from '../rest.util';
import type { AdoptionDecision, AdoptionRequest } from '../model.generated';

@Injectable({
  providedIn: 'root',
})
export class AdoptionRequestRest {

  readonly createAdoptionRequest = requestFactory<CreateAdoptionRequestParams, AdoptionDecision>(
    (request: CreateAdoptionRequestParams) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/adoption-request`,
        body: body,
      };
    },
  );
}

export interface CreateAdoptionRequestParams {
  body: AdoptionRequest;
}
