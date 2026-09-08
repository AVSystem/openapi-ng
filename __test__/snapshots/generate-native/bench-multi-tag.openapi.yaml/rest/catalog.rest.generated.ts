import { Injectable } from '@angular/core';
import { httpParams, requestFactory } from '../rest.util';
import type {
  CatalogItem1,
  CatalogItem10,
  CatalogItem10List,
  CatalogItem1List,
  CatalogItem2,
  CatalogItem2List,
  CatalogItem3,
  CatalogItem3List,
  CatalogItem4,
  CatalogItem4List,
  CatalogItem5,
  CatalogItem5List,
  CatalogItem6,
  CatalogItem6List,
  CatalogItem7,
  CatalogItem7List,
  CatalogItem8,
  CatalogItem8List,
  CatalogItem9,
  CatalogItem9List,
} from '../model.generated';

@Injectable({
  providedIn: 'root',
})
export class CatalogRest {

  readonly createCatalogItem1 = requestFactory<CreateCatalogItem1Params, CatalogItem1>(
    (request: CreateCatalogItem1Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/catalog/1`,
        body: body,
      };
    },
  );

  readonly createCatalogItem10 = requestFactory<CreateCatalogItem10Params, CatalogItem10>(
    (request: CreateCatalogItem10Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/catalog/10`,
        body: body,
      };
    },
  );

  readonly createCatalogItem2 = requestFactory<CreateCatalogItem2Params, CatalogItem2>(
    (request: CreateCatalogItem2Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/catalog/2`,
        body: body,
      };
    },
  );

  readonly createCatalogItem3 = requestFactory<CreateCatalogItem3Params, CatalogItem3>(
    (request: CreateCatalogItem3Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/catalog/3`,
        body: body,
      };
    },
  );

  readonly createCatalogItem4 = requestFactory<CreateCatalogItem4Params, CatalogItem4>(
    (request: CreateCatalogItem4Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/catalog/4`,
        body: body,
      };
    },
  );

  readonly createCatalogItem5 = requestFactory<CreateCatalogItem5Params, CatalogItem5>(
    (request: CreateCatalogItem5Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/catalog/5`,
        body: body,
      };
    },
  );

  readonly createCatalogItem6 = requestFactory<CreateCatalogItem6Params, CatalogItem6>(
    (request: CreateCatalogItem6Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/catalog/6`,
        body: body,
      };
    },
  );

  readonly createCatalogItem7 = requestFactory<CreateCatalogItem7Params, CatalogItem7>(
    (request: CreateCatalogItem7Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/catalog/7`,
        body: body,
      };
    },
  );

  readonly createCatalogItem8 = requestFactory<CreateCatalogItem8Params, CatalogItem8>(
    (request: CreateCatalogItem8Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/catalog/8`,
        body: body,
      };
    },
  );

  readonly createCatalogItem9 = requestFactory<CreateCatalogItem9Params, CatalogItem9>(
    (request: CreateCatalogItem9Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/catalog/9`,
        body: body,
      };
    },
  );

  readonly getCatalogItem1 = requestFactory<GetCatalogItem1Params, CatalogItem1List>(
    (request: GetCatalogItem1Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/catalog/1`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getCatalogItem10 = requestFactory<GetCatalogItem10Params, CatalogItem10List>(
    (request: GetCatalogItem10Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/catalog/10`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getCatalogItem2 = requestFactory<GetCatalogItem2Params, CatalogItem2List>(
    (request: GetCatalogItem2Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/catalog/2`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getCatalogItem3 = requestFactory<GetCatalogItem3Params, CatalogItem3List>(
    (request: GetCatalogItem3Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/catalog/3`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getCatalogItem4 = requestFactory<GetCatalogItem4Params, CatalogItem4List>(
    (request: GetCatalogItem4Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/catalog/4`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getCatalogItem5 = requestFactory<GetCatalogItem5Params, CatalogItem5List>(
    (request: GetCatalogItem5Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/catalog/5`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getCatalogItem6 = requestFactory<GetCatalogItem6Params, CatalogItem6List>(
    (request: GetCatalogItem6Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/catalog/6`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getCatalogItem7 = requestFactory<GetCatalogItem7Params, CatalogItem7List>(
    (request: GetCatalogItem7Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/catalog/7`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getCatalogItem8 = requestFactory<GetCatalogItem8Params, CatalogItem8List>(
    (request: GetCatalogItem8Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/catalog/8`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getCatalogItem9 = requestFactory<GetCatalogItem9Params, CatalogItem9List>(
    (request: GetCatalogItem9Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/catalog/9`,
        params: httpParams({ limit }),
      };
    },
  );
}

export interface CreateCatalogItem1Params {
  body: CatalogItem1;
}

export interface CreateCatalogItem10Params {
  body: CatalogItem10;
}

export interface CreateCatalogItem2Params {
  body: CatalogItem2;
}

export interface CreateCatalogItem3Params {
  body: CatalogItem3;
}

export interface CreateCatalogItem4Params {
  body: CatalogItem4;
}

export interface CreateCatalogItem5Params {
  body: CatalogItem5;
}

export interface CreateCatalogItem6Params {
  body: CatalogItem6;
}

export interface CreateCatalogItem7Params {
  body: CatalogItem7;
}

export interface CreateCatalogItem8Params {
  body: CatalogItem8;
}

export interface CreateCatalogItem9Params {
  body: CatalogItem9;
}

export interface GetCatalogItem1Params {
  limit?: number;
}

export interface GetCatalogItem10Params {
  limit?: number;
}

export interface GetCatalogItem2Params {
  limit?: number;
}

export interface GetCatalogItem3Params {
  limit?: number;
}

export interface GetCatalogItem4Params {
  limit?: number;
}

export interface GetCatalogItem5Params {
  limit?: number;
}

export interface GetCatalogItem6Params {
  limit?: number;
}

export interface GetCatalogItem7Params {
  limit?: number;
}

export interface GetCatalogItem8Params {
  limit?: number;
}

export interface GetCatalogItem9Params {
  limit?: number;
}
