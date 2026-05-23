export interface Cat {
  kind: 'cat';
  lives: number;
}

export interface Dog {
  kind: 'dog';
  breed: string;
}

export type PetUnion = Cat | Dog;
