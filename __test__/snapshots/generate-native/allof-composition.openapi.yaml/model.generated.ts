export type AdopterProfile = AuditFields & ContactFields & {
  id: string;
  name: string;
  nickname?: string | null;
};

export interface AuditFields {
  createdAt: string;
  archivedAt?: string | null;
}

export interface ContactFields {
  email: string;
  phone?: string;
}
