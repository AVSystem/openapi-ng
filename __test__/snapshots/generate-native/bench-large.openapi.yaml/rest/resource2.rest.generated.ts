import { Injectable } from '@angular/core';
import { httpParams, requestFactory } from '../rest.util';
import type {
  Resource10,
  Resource10List,
  Resource6,
  Resource6List,
  Resource7,
  Resource7List,
  Resource8,
  Resource8List,
  Resource9,
  Resource9List,
} from '../model.generated';

@Injectable({
  providedIn: 'root',
})
export class Resource2Rest {

  readonly createResource10 = requestFactory<CreateResource10Params, Resource10>(
    (request: CreateResource10Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/resource10`,
        body: body,
      };
    },
  );

  readonly createResource6 = requestFactory<CreateResource6Params, Resource6>(
    (request: CreateResource6Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/resource6`,
        body: body,
      };
    },
  );

  readonly createResource7 = requestFactory<CreateResource7Params, Resource7>(
    (request: CreateResource7Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/resource7`,
        body: body,
      };
    },
  );

  readonly createResource8 = requestFactory<CreateResource8Params, Resource8>(
    (request: CreateResource8Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/resource8`,
        body: body,
      };
    },
  );

  readonly createResource9 = requestFactory<CreateResource9Params, Resource9>(
    (request: CreateResource9Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/resource9`,
        body: body,
      };
    },
  );

  readonly getResource10 = requestFactory<GetResource10Params, Resource10List>(
    (request: GetResource10Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/resource10`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getResource6 = requestFactory<GetResource6Params, Resource6List>(
    (request: GetResource6Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/resource6`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getResource7 = requestFactory<GetResource7Params, Resource7List>(
    (request: GetResource7Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/resource7`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getResource8 = requestFactory<GetResource8Params, Resource8List>(
    (request: GetResource8Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/resource8`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getResource9 = requestFactory<GetResource9Params, Resource9List>(
    (request: GetResource9Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/resource9`,
        params: httpParams({ limit }),
      };
    },
  );
}

export interface CreateResource10Params {
  body: Resource10;
}

export interface CreateResource6Params {
  body: Resource6;
}

export interface CreateResource7Params {
  body: Resource7;
}

export interface CreateResource8Params {
  body: Resource8;
}

export interface CreateResource9Params {
  body: Resource9;
}

export interface GetResource10Params {
  limit?: number;
}

export interface GetResource6Params {
  limit?: number;
}

export interface GetResource7Params {
  limit?: number;
}

export interface GetResource8Params {
  limit?: number;
}

export interface GetResource9Params {
  limit?: number;
}
