import { Injectable } from '@angular/core';
import { requestFactory } from '../rest.util';
import type { PetUnionList } from '../model';

@Injectable({
  providedIn: 'root',
})
export class PetRest {

  readonly listUnionPets = requestFactory.zeroArg<PetUnionList>(
    () => ({
      method: 'GET',
      url: `/union-pets`,
    }),
  );
}
