import { Injectable } from '@angular/core';
import { httpParams, requestFactory } from '../rest.util';
import type {
  Resource11,
  Resource11List,
  Resource12,
  Resource12List,
  Resource13,
  Resource13List,
  Resource14,
  Resource14List,
  Resource15,
  Resource15List,
} from '../model.generated';

@Injectable({
  providedIn: 'root',
})
export class Resource3Rest {

  readonly createResource11 = requestFactory<CreateResource11Params, Resource11>(
    (request: CreateResource11Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/resource11`,
        body: body,
      };
    },
  );

  readonly createResource12 = requestFactory<CreateResource12Params, Resource12>(
    (request: CreateResource12Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/resource12`,
        body: body,
      };
    },
  );

  readonly createResource13 = requestFactory<CreateResource13Params, Resource13>(
    (request: CreateResource13Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/resource13`,
        body: body,
      };
    },
  );

  readonly createResource14 = requestFactory<CreateResource14Params, Resource14>(
    (request: CreateResource14Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/resource14`,
        body: body,
      };
    },
  );

  readonly createResource15 = requestFactory<CreateResource15Params, Resource15>(
    (request: CreateResource15Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/resource15`,
        body: body,
      };
    },
  );

  readonly getResource11 = requestFactory<GetResource11Params, Resource11List>(
    (request: GetResource11Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/resource11`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getResource12 = requestFactory<GetResource12Params, Resource12List>(
    (request: GetResource12Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/resource12`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getResource13 = requestFactory<GetResource13Params, Resource13List>(
    (request: GetResource13Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/resource13`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getResource14 = requestFactory<GetResource14Params, Resource14List>(
    (request: GetResource14Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/resource14`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getResource15 = requestFactory<GetResource15Params, Resource15List>(
    (request: GetResource15Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/resource15`,
        params: httpParams({ limit }),
      };
    },
  );
}

export interface CreateResource11Params {
  body: Resource11;
}

export interface CreateResource12Params {
  body: Resource12;
}

export interface CreateResource13Params {
  body: Resource13;
}

export interface CreateResource14Params {
  body: Resource14;
}

export interface CreateResource15Params {
  body: Resource15;
}

export interface GetResource11Params {
  limit?: number;
}

export interface GetResource12Params {
  limit?: number;
}

export interface GetResource13Params {
  limit?: number;
}

export interface GetResource14Params {
  limit?: number;
}

export interface GetResource15Params {
  limit?: number;
}
