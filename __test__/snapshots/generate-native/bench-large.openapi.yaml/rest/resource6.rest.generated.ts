import { Injectable } from '@angular/core';
import { httpParams, requestFactory } from '../rest.util';
import type {
  Resource26,
  Resource26List,
  Resource27,
  Resource27List,
  Resource28,
  Resource28List,
  Resource29,
  Resource29List,
  Resource30,
  Resource30List,
} from '../model.generated';

@Injectable({
  providedIn: 'root',
})
export class Resource6Rest {

  readonly createResource26 = requestFactory<CreateResource26Params, Resource26>(
    (request: CreateResource26Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/resource26`,
        body: body,
      };
    },
  );

  readonly createResource27 = requestFactory<CreateResource27Params, Resource27>(
    (request: CreateResource27Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/resource27`,
        body: body,
      };
    },
  );

  readonly createResource28 = requestFactory<CreateResource28Params, Resource28>(
    (request: CreateResource28Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/resource28`,
        body: body,
      };
    },
  );

  readonly createResource29 = requestFactory<CreateResource29Params, Resource29>(
    (request: CreateResource29Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/resource29`,
        body: body,
      };
    },
  );

  readonly createResource30 = requestFactory<CreateResource30Params, Resource30>(
    (request: CreateResource30Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/resource30`,
        body: body,
      };
    },
  );

  readonly getResource26 = requestFactory<GetResource26Params, Resource26List>(
    (request: GetResource26Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/resource26`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getResource27 = requestFactory<GetResource27Params, Resource27List>(
    (request: GetResource27Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/resource27`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getResource28 = requestFactory<GetResource28Params, Resource28List>(
    (request: GetResource28Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/resource28`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getResource29 = requestFactory<GetResource29Params, Resource29List>(
    (request: GetResource29Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/resource29`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getResource30 = requestFactory<GetResource30Params, Resource30List>(
    (request: GetResource30Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/resource30`,
        params: httpParams({ limit }),
      };
    },
  );
}

export interface CreateResource26Params {
  body: Resource26;
}

export interface CreateResource27Params {
  body: Resource27;
}

export interface CreateResource28Params {
  body: Resource28;
}

export interface CreateResource29Params {
  body: Resource29;
}

export interface CreateResource30Params {
  body: Resource30;
}

export interface GetResource26Params {
  limit?: number;
}

export interface GetResource27Params {
  limit?: number;
}

export interface GetResource28Params {
  limit?: number;
}

export interface GetResource29Params {
  limit?: number;
}

export interface GetResource30Params {
  limit?: number;
}
