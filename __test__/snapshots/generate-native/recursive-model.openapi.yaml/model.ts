export interface Category {
  name: string;
  subcategories: Category[];
}

export interface Person {
  name: string;
  favoritePet?: Pet;
}

export interface Pet {
  name: string;
  owner?: Person;
}
