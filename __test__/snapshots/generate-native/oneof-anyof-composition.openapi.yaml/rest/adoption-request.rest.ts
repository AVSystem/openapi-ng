import { Injectable } from '@angular/core';
import { mutationFactory } from '../rest.util';
import type { AdoptionDecision, AdoptionRequest } from '../model';

@Injectable({
  providedIn: 'root',
})
export class AdoptionRequestRest {

  readonly createAdoptionRequest = mutationFactory<CreateAdoptionRequestParams, AdoptionDecision>(
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
