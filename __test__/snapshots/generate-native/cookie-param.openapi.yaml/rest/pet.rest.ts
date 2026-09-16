import { Injectable } from '@angular/core';
import { requestFactory } from '../rest.util';

@Injectable({
  providedIn: 'root',
})
export class PetRest {

  readonly listPets = requestFactory.zeroArg<void>(
    () => ({
      method: 'GET',
      url: `/pets`,
    }),
  );
}
