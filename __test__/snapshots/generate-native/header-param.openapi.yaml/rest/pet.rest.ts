import { Injectable } from '@angular/core';
import { requestFactory } from '../rest.util';

@Injectable({
  providedIn: 'root',
})
export class PetRest {

  readonly listPets = requestFactory<ListPetsParams, void>(
    (request: ListPetsParams) => {
      const { headers } = request;
      return {
        method: 'GET',
        url: `/pets`,
        headers,
      };
    },
  );
}

export interface ListPetsParams {
  headers: {
    'X-Api-Key': string;
  };
}
