/**
 * Adoption state, legacy spelling.
 * @deprecated
 */
export type LegacyPetStatus = 'available' | 'sold';

export interface Pet {
  id: string;
  /**
   * Legacy numeric tag id; use `tagIds` instead.
   * @deprecated
   */
  legacyTagId?: number;
  tagIds?: number[];
}
