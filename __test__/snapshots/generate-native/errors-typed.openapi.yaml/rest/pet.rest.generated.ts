import { Injectable } from '@angular/core';
import { requestFactory } from '../rest.util';
import type { NotFound, Pet, UpdatePetRequest, ValidationProblem } from '../model.generated';

@Injectable({
  providedIn: 'root',
})
export class PetRest {

  readonly updatePet = requestFactory<UpdatePetParams, Pet>(
    (request: UpdatePetParams) => {
      const { petId, body } = request;
      return {
        method: 'POST',
        url: `/pets/${encodeURIComponent(petId)}`,
        body: body,
      };
    },
  );
}

export interface UpdatePetParams {
  petId: string;
  body: UpdatePetRequest;
}

export interface UpdatePetError {
  400: ValidationProblem;
  404: NotFound;
  500: {
    traceId: string;
  };
}
