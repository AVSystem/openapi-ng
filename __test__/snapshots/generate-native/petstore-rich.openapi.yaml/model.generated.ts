export interface Pet {
  id: PetId;
  name: string;
  status: PetStatus;
  tags: Tag[];
  nickname?: string | null;
}

export type PetId = string;

export type PetList = Pet[];

export type PetStatus = 'available' | 'pending' | 'sold';

export interface Tag {
  id: number;
  label: string;
}

export interface UpdatePetRequest {
  status: PetStatus;
  tagIds: number[];
  nickname?: string | null;
}
