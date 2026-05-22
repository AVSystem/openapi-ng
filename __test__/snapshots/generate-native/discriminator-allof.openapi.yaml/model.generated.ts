export interface Animal {
  name: string;
}

export type Cat = Animal & {
  kind: 'cat';
  whiskers?: number;
};

export type Dog = Animal & {
  kind: 'dog';
  barkLoudness?: string;
};

export type Pet = Cat | Dog;
