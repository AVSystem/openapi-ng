export interface Pet {
  id: string;
  name: string;
}

export interface PetCatalog {
  scope: 'available' | 'adopted' | 'foster';
  petsByBreed: Record<string, Pet[]>;
}

export interface PetMetadata {
  spotlight: boolean;
  tag: Tag;
}

export type PetMetadataByTag = Record<string, PetMetadata>;

export interface Tag {
  id: number;
  label: string;
}
