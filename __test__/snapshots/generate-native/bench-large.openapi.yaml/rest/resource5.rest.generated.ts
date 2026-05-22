import { Injectable } from '@angular/core';
import { httpParams, requestFactory } from '../rest.util';
import type {
  Resource21,
  Resource21List,
  Resource22,
  Resource22List,
  Resource23,
  Resource23List,
  Resource24,
  Resource24List,
  Resource25,
  Resource25List,
} from '../model.generated';

@Injectable({
  providedIn: 'root',
})
export class Resource5Rest {

  readonly createResource21 = requestFactory<CreateResource21Params, Resource21>(
    (request: CreateResource21Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/resource21`,
        body: body,
      };
    },
  );

  readonly createResource22 = requestFactory<CreateResource22Params, Resource22>(
    (request: CreateResource22Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/resource22`,
        body: body,
      };
    },
  );

  readonly createResource23 = requestFactory<CreateResource23Params, Resource23>(
    (request: CreateResource23Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/resource23`,
        body: body,
      };
    },
  );

  readonly createResource24 = requestFactory<CreateResource24Params, Resource24>(
    (request: CreateResource24Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/resource24`,
        body: body,
      };
    },
  );

  readonly createResource25 = requestFactory<CreateResource25Params, Resource25>(
    (request: CreateResource25Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/resource25`,
        body: body,
      };
    },
  );

  readonly getResource21 = requestFactory<GetResource21Params, Resource21List>(
    (request: GetResource21Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/resource21`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getResource22 = requestFactory<GetResource22Params, Resource22List>(
    (request: GetResource22Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/resource22`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getResource23 = requestFactory<GetResource23Params, Resource23List>(
    (request: GetResource23Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/resource23`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getResource24 = requestFactory<GetResource24Params, Resource24List>(
    (request: GetResource24Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/resource24`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getResource25 = requestFactory<GetResource25Params, Resource25List>(
    (request: GetResource25Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/resource25`,
        params: httpParams({ limit }),
      };
    },
  );
}

export interface CreateResource21Params {
  body: Resource21;
}

export interface CreateResource22Params {
  body: Resource22;
}

export interface CreateResource23Params {
  body: Resource23;
}

export interface CreateResource24Params {
  body: Resource24;
}

export interface CreateResource25Params {
  body: Resource25;
}

export interface GetResource21Params {
  limit?: number;
}

export interface GetResource22Params {
  limit?: number;
}

export interface GetResource23Params {
  limit?: number;
}

export interface GetResource24Params {
  limit?: number;
}

export interface GetResource25Params {
  limit?: number;
}
