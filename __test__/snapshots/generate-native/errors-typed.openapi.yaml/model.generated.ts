export interface NotFound {
  resource: string;
}

export interface Pet {
  id: string;
}

export interface UpdatePetRequest {
  status: string;
}

export interface ValidationProblem {
  code: string;
  field: string;
}
