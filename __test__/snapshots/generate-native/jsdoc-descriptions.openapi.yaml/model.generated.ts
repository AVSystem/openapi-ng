/**
 * A pet that is up for adoption.
 */
export interface Pet {
  /**
   * Stable identifier across renames.
   */
  id: string;
  status: PetStatus;
  /**
   * Optional informal name used in marketing copy.
   */
  nickname?: string;
}

/**
 * Adoption state of the pet.
 */
export type PetStatus = 'available' | 'pending' | 'adopted';
