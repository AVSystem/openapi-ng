import { Injectable } from '@angular/core';
import { requestFactory } from '../rest.util';
import type { Pet } from '../model.generated';

@Injectable({
  providedIn: 'root',
})
export class PetRest {

  readonly listPets = requestFactory.zeroArg<Pet>(
    () => ({
      method: 'GET',
      url: `/pets`,
    }),
  );
}
