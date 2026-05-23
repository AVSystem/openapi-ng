export interface PetProfile {
  id: string;
  details: {
    displayName: string;
    address: {
      city: string;
      postalCode?: string | null;
    };
  };
  labelsByLocale: Record<string, {
    value: string;
  }>;
  visits: {
    visitedAt: string;
    notes?: string | null;
  }[];
}
