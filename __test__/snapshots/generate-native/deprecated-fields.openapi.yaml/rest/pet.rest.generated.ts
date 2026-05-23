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
   * Use `getPetById` instead — this endpoint will be removed.
   * @deprecated
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
