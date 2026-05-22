export interface Cat {
  kind: 'feline';
}

export interface Dog {
  kind: 'canine';
}

export type Pet = Cat | Dog;
