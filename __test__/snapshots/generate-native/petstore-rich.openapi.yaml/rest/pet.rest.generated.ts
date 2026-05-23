import { Injectable } from '@angular/core';
import { httpParams, requestFactory } from '../rest.util';
import type { Pet, PetId, PetList, UpdatePetRequest } from '../model.generated';

@Injectable({
  providedIn: 'root',
})
export class PetRest {

  readonly getPet = requestFactory<GetPetParams, Pet>(
    (request: GetPetParams) => {
      const { petId } = request;
      return {
        method: 'GET',
        url: `/pets/${encodeURIComponent(petId)}`,
      };
    },
  );

  readonly listPets = requestFactory.zeroArg<PetList>(
    () => ({
      method: 'GET',
      url: `/pets`,
    }),
  );

  readonly updatePet = requestFactory<UpdatePetParams, Pet>(
    (request: UpdatePetParams) => {
      const { petId, includeHistory, body } = request;
      return {
        method: 'POST',
        url: `/pets/${encodeURIComponent(petId)}`,
        params: httpParams({ includeHistory }),
        body: body,
      };
    },
  );
}

export interface GetPetParams {
  petId: PetId;
}

export interface UpdatePetParams {
  petId: PetId;
  includeHistory?: boolean;
  body: UpdatePetRequest;
}
