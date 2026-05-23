import { Injectable } from '@angular/core';
import { httpParams, requestFactory } from '../rest.util';
import type {
  Resource16,
  Resource16List,
  Resource17,
  Resource17List,
  Resource18,
  Resource18List,
  Resource19,
  Resource19List,
  Resource20,
  Resource20List,
} from '../model.generated';

@Injectable({
  providedIn: 'root',
})
export class Resource4Rest {

  readonly createResource16 = requestFactory<CreateResource16Params, Resource16>(
    (request: CreateResource16Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/resource16`,
        body: body,
      };
    },
  );

  readonly createResource17 = requestFactory<CreateResource17Params, Resource17>(
    (request: CreateResource17Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/resource17`,
        body: body,
      };
    },
  );

  readonly createResource18 = requestFactory<CreateResource18Params, Resource18>(
    (request: CreateResource18Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/resource18`,
        body: body,
      };
    },
  );

  readonly createResource19 = requestFactory<CreateResource19Params, Resource19>(
    (request: CreateResource19Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/resource19`,
        body: body,
      };
    },
  );

  readonly createResource20 = requestFactory<CreateResource20Params, Resource20>(
    (request: CreateResource20Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/resource20`,
        body: body,
      };
    },
  );

  readonly getResource16 = requestFactory<GetResource16Params, Resource16List>(
    (request: GetResource16Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/resource16`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getResource17 = requestFactory<GetResource17Params, Resource17List>(
    (request: GetResource17Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/resource17`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getResource18 = requestFactory<GetResource18Params, Resource18List>(
    (request: GetResource18Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/resource18`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getResource19 = requestFactory<GetResource19Params, Resource19List>(
    (request: GetResource19Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/resource19`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getResource20 = requestFactory<GetResource20Params, Resource20List>(
    (request: GetResource20Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/resource20`,
        params: httpParams({ limit }),
      };
    },
  );
}

export interface CreateResource16Params {
  body: Resource16;
}

export interface CreateResource17Params {
  body: Resource17;
}

export interface CreateResource18Params {
  body: Resource18;
}

export interface CreateResource19Params {
  body: Resource19;
}

export interface CreateResource20Params {
  body: Resource20;
}

export interface GetResource16Params {
  limit?: number;
}

export interface GetResource17Params {
  limit?: number;
}

export interface GetResource18Params {
  limit?: number;
}

export interface GetResource19Params {
  limit?: number;
}

export interface GetResource20Params {
  limit?: number;
}
