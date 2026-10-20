import { Injectable } from '@angular/core';
import { mutationFactory, requestFactory } from '../rest.util';
import type { Pet } from '../model';

@Injectable({
  providedIn: 'root',
})
export class PetRest {

  readonly createPet = mutationFactory<CreatePetParams, Pet>(
    (request: CreatePetParams) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/pets`,
        body: body,
      };
    },
  );

  readonly deletePet = requestFactory<DeletePetParams, void>(
    (request: DeletePetParams) => {
      const { petId } = request;
      return {
        method: 'DELETE',
        url: `/pets/${encodeURIComponent(petId)}`,
      };
    },
  );

  readonly exportPet = mutationFactory.blob<ExportPetParams>(
    (request: ExportPetParams) => {
      const { petId } = request;
      return {
        method: 'POST',
        url: `/pets/${encodeURIComponent(petId)}/export`,
      };
    },
  );

  readonly listPets = mutationFactory.zeroArg<Pet[]>(
    () => ({
      method: 'GET',
      url: `/pets`,
    }),
  );

  readonly previewDeletePet = requestFactory<PreviewDeletePetParams, Pet>(
    (request: PreviewDeletePetParams) => {
      const { petId } = request;
      return {
        method: 'DELETE',
        url: `/pets/${encodeURIComponent(petId)}/preview-delete`,
      };
    },
  );

  readonly renderReport = mutationFactory.zeroArg.text(
    () => ({
      method: 'POST',
      url: `/reports/render`,
    }),
  );

  readonly searchPets = requestFactory<SearchPetsParams, Pet[]>(
    (request: SearchPetsParams) => {
      const { name } = request;
      return {
        method: 'POST',
        url: `/pets/search`,
        body: { name },
      };
    },
  );
}

export interface CreatePetParams {
  body: Pet;
}

export interface DeletePetParams {
  petId: string;
}

export interface ExportPetParams {
  petId: string;
}

export interface PreviewDeletePetParams {
  petId: string;
}

export interface SearchPetsParams {
  name?: string;
}
