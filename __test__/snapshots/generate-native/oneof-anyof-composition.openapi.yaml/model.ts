export interface AdoptionDecision {
  approved: boolean;
  matchedPet?: PetUnion;
  reviewerNote?: string | null;
}

export interface AdoptionRequest {
  applicantName: string;
  preferredPet?: PetUnion;
  contact: ContactEmail | ContactPhone;
  notes?: string;
  referralCode?: string | null;
}

export interface Cat {
  id: string;
  lives: number;
}

export interface ContactEmail {
  email: string;
}

export interface ContactPhone {
  phone: string;
}

export interface Dog {
  id: string;
  breed: string;
}

export type PetUnion = Cat | Dog;

export type PetUnionList = PetUnion[];
