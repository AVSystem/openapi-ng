import { Injectable } from '@angular/core';
import { httpParams, requestFactory } from '../rest.util';
import type {
  Resource1,
  Resource1List,
  Resource2,
  Resource2List,
  Resource3,
  Resource3List,
  Resource4,
  Resource4List,
  Resource5,
  Resource5List,
} from '../model.generated';

@Injectable({
  providedIn: 'root',
})
export class Resource1Rest {

  readonly createResource1 = requestFactory<CreateResource1Params, Resource1>(
    (request: CreateResource1Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/resource1`,
        body: body,
      };
    },
  );

  readonly createResource2 = requestFactory<CreateResource2Params, Resource2>(
    (request: CreateResource2Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/resource2`,
        body: body,
      };
    },
  );

  readonly createResource3 = requestFactory<CreateResource3Params, Resource3>(
    (request: CreateResource3Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/resource3`,
        body: body,
      };
    },
  );

  readonly createResource4 = requestFactory<CreateResource4Params, Resource4>(
    (request: CreateResource4Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/resource4`,
        body: body,
      };
    },
  );

  readonly createResource5 = requestFactory<CreateResource5Params, Resource5>(
    (request: CreateResource5Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/resource5`,
        body: body,
      };
    },
  );

  readonly getResource1 = requestFactory<GetResource1Params, Resource1List>(
    (request: GetResource1Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/resource1`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getResource2 = requestFactory<GetResource2Params, Resource2List>(
    (request: GetResource2Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/resource2`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getResource3 = requestFactory<GetResource3Params, Resource3List>(
    (request: GetResource3Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/resource3`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getResource4 = requestFactory<GetResource4Params, Resource4List>(
    (request: GetResource4Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/resource4`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getResource5 = requestFactory<GetResource5Params, Resource5List>(
    (request: GetResource5Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/resource5`,
        params: httpParams({ limit }),
      };
    },
  );
}

export interface CreateResource1Params {
  body: Resource1;
}

export interface CreateResource2Params {
  body: Resource2;
}

export interface CreateResource3Params {
  body: Resource3;
}

export interface CreateResource4Params {
  body: Resource4;
}

export interface CreateResource5Params {
  body: Resource5;
}

export interface GetResource1Params {
  limit?: number;
}

export interface GetResource2Params {
  limit?: number;
}

export interface GetResource3Params {
  limit?: number;
}

export interface GetResource4Params {
  limit?: number;
}

export interface GetResource5Params {
  limit?: number;
}
