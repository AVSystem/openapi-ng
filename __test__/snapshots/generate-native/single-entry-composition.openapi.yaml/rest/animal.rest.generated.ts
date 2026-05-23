import { Injectable } from '@angular/core';
import { requestFactory } from '../rest.util';
import type { AnimalView } from '../model.generated';

@Injectable({
  providedIn: 'root',
})
export class AnimalRest {

  readonly getAnimal = requestFactory<GetAnimalParams, AnimalView>(
    (request: GetAnimalParams) => {
      const { animalId } = request;
      return {
        method: 'GET',
        url: `/animals/${encodeURIComponent(animalId)}`,
      };
    },
  );
}

export interface GetAnimalParams {
  animalId: string;
}
