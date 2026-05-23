export interface Cat {
  kind: 'cat';
  whiskers?: number;
}

export interface Dog {
  kind: 'dog';
  breed?: string;
}

export type Pet = Cat | Dog | null;

export interface Profile {
  id: string;
  favoritePet?: Cat | Dog | null;
}
