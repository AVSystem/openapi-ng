import { Injectable } from '@angular/core';
import * as ops from './pet';

@Injectable({
  providedIn: 'root',
})
export class PetRest {
  readonly getPet = ops.getPet.withInjector();
  readonly listPets = ops.listPets.withInjector();
  readonly updatePet = ops.updatePet.withInjector();
}

export type { GetPetParams, UpdatePetParams } from './pet';
