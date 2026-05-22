import { Injectable } from '@angular/core';
import { requestFactory } from '../rest.util';
import type { Pet } from '../model.generated';

@Injectable({
  providedIn: 'root',
})
export class PetRest {

  /**
   * Retrieve a single pet
   *
   * Returns the full pet record by id. The returned shape
   * matches `Pet` exactly — no partial fetches.
   */
  readonly getPet = requestFactory<GetPetParams, Pet>(
    (request: GetPetParams) => {
      const { petId } = request;
      return {
        method: 'GET',
        url: `/pets/${encodeURIComponent(petId)}`,
      };
    },
  );
}

export interface GetPetParams {
  petId: string;
}
