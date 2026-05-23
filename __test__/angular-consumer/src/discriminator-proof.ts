import type { Cat, Dog, PetUnion } from '../generated/model.generated';

declare const pet: PetUnion;

// These narrowing blocks only type-check if Cat.kind = 'cat' and Dog.kind = 'dog'
// (literal types). Without the discriminator narrowing the compiler cannot
// prove that pet.lives / pet.breed are accessible inside the if block.

if (pet.kind === 'cat') {
  const lives: number = pet.lives;
  void lives;
}

if (pet.kind === 'dog') {
  const breed: string = pet.breed;
  void breed;
}

// Exhaustiveness check — both branches must be accounted for.
function assertNever(x: never): never {
  throw new Error(`Unexpected value: ${x}`);
}

function describePet(p: PetUnion): string {
  switch (p.kind) {
    case 'cat':
      return `cat with ${p.lives} lives`;
    case 'dog':
      return `dog of breed ${p.breed}`;
    default:
      return assertNever(p);
  }
}

declare const cat: Cat;
declare const dog: Dog;
void describePet(cat);
void describePet(dog);
