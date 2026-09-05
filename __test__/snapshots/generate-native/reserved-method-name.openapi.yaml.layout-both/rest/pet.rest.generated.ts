import { Injectable } from '@angular/core';
import * as ops from './pet.operations.generated';

@Injectable({
  providedIn: 'root',
})
export class PetRest {
  /**
   * Remove a pet. The operationId is a reserved word on purpose.
   */
  readonly delete = ops.delete.withInjector();

  readonly getPet = ops.getPet.withInjector();

  /**
   * List pets, optionally filtered by status.
   */
  readonly listPets = ops.listPets.withInjector();
}

export type {
  DeleteParams,
  GetPetParams,
  GetPetError,
  ListPetsParams,
} from './pet.operations.generated';
