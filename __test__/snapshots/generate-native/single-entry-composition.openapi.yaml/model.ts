export interface AnimalBase {
  id: string;
  name: string;
  nickname?: string | null;
}

export type AnimalDraft = AnimalBase;

export type AnimalView = AnimalBase;

export type ContactPreference = string;
