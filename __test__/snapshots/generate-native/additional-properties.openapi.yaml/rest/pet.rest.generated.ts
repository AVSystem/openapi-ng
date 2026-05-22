import { Injectable } from '@angular/core';
import { requestFactory } from '../rest.util';
import type { PetCatalog } from '../model.generated';

@Injectable({
  providedIn: 'root',
})
export class PetRest {

  readonly listPetCatalog = requestFactory.zeroArg<PetCatalog>(
    () => ({
      method: 'GET',
      url: `/pets`,
    }),
  );
}
