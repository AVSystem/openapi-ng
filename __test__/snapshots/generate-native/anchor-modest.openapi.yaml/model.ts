export interface Audit {
  createdAt?: string;
  updatedAt?: string;
}

export type Owner = {
  createdAt?: string;
  updatedAt?: string;
} & {
  id?: string;
  displayName?: string;
};

export type Pet = {
  createdAt?: string;
  updatedAt?: string;
} & {
  id?: string;
  name?: string;
};

export type Visit = {
  createdAt?: string;
  updatedAt?: string;
} & {
  id?: string;
  notes?: string;
};
