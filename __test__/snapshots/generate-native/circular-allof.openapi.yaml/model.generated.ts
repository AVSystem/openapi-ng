export interface BaseAuditFields {
  createdAt: string;
  updatedAt?: string | null;
}

export type DeepRecord = Layer4 & {
  tier5?: string;
};

export type Layer1 = BaseAuditFields & {
  tier1?: string;
};

export type Layer2 = Layer1 & {
  tier2?: string;
};

export type Layer3 = Layer2 & {
  tier3?: string;
};

export type Layer4 = Layer3 & {
  tier4?: string;
};
