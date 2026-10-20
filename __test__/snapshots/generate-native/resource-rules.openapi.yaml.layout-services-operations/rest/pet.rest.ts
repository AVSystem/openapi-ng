import { Injectable } from '@angular/core';
import * as ops from './pet';

@Injectable({
  providedIn: 'root',
})
export class PetRest {
  readonly createPet = ops.createPet.withInjector();
  readonly deletePet = ops.deletePet.withInjector();
  readonly exportPet = ops.exportPet.withInjector();
  readonly listPets = ops.listPets.withInjector();
  readonly previewDeletePet = ops.previewDeletePet.withInjector();
  readonly renderReport = ops.renderReport.withInjector();
  readonly searchPets = ops.searchPets.withInjector();
}

export type {
  CreatePetParams,
  DeletePetParams,
  ExportPetParams,
  PreviewDeletePetParams,
  SearchPetsParams,
} from './pet';
