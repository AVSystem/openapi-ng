import type { HttpClient } from '@angular/common/http';

import type {
  AdoptionDecision,
  AdoptionRequest,
  ContactEmail,
  ContactPhone,
  PetUnion,
  PetUnionList,
} from '../generated/model.generated';

declare const http: HttpClient;

declare function expectType<T>(value: T): void;

const emailContact: ContactEmail = { email: 'owner@example.com' };
const phoneContact: ContactPhone = { phone: '+48123456789' };
const preferredPet: PetUnion = {
  id: 'pet-123',
  lives: 7,
};

const listUnionPets: PetUnionList = [preferredPet];

const nullableFreeRequest: AdoptionRequest = {
  applicantName: 'Jordan Example',
  contact: emailContact,
  preferredPet,
  notes: 'Has a fenced yard',
  referralCode: 'SPRING-2026',
};

const applicantWithEmail: AdoptionRequest = {
  applicantName: 'Taylor Example',
  contact: emailContact,
  preferredPet,
  notes: 'Works from home',
  referralCode: 'FRIEND',
};

const applicantWithPhone: AdoptionRequest = {
  applicantName: 'Morgan Example',
  contact: phoneContact,
};

const approvedDecision: AdoptionDecision = {
  approved: true,
  matchedPet: preferredPet,
  reviewerNote: 'Bring carrier to pickup',
};

const pendingDecision: AdoptionDecision = {
  approved: false,
};

expectType<ContactEmail | ContactPhone>(applicantWithPhone.contact);
expectType<PetUnion | undefined>(applicantWithEmail.preferredPet);
expectType<string | null | undefined>(approvedDecision.reviewerNote);
expectType<PetUnion | undefined>(approvedDecision.matchedPet);
expectType<PetUnionList>(listUnionPets);

void http;
void nullableFreeRequest;
void approvedDecision;
void pendingDecision;
