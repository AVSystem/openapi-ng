import { Injectable } from '@angular/core';
import { requestFactory } from '../rest.util';
import type { PetList } from '../model.generated';

@Injectable({
  providedIn: 'root',
})
export class PetRest {

  readonly listPetsAndShelters = requestFactory.zeroArg<PetList>(
    () => ({
      method: 'GET',
      url: `/pets`,
    }),
  );
}
