import { Injectable } from '@angular/core';
import { httpParams, requestFactory } from '../rest.util';
import type {
  InventoryItem1,
  InventoryItem10,
  InventoryItem10List,
  InventoryItem1List,
  InventoryItem2,
  InventoryItem2List,
  InventoryItem3,
  InventoryItem3List,
  InventoryItem4,
  InventoryItem4List,
  InventoryItem5,
  InventoryItem5List,
  InventoryItem6,
  InventoryItem6List,
  InventoryItem7,
  InventoryItem7List,
  InventoryItem8,
  InventoryItem8List,
  InventoryItem9,
  InventoryItem9List,
} from '../model.generated';

@Injectable({
  providedIn: 'root',
})
export class InventoryRest {

  readonly createInventoryItem1 = requestFactory<CreateInventoryItem1Params, InventoryItem1>(
    (request: CreateInventoryItem1Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/inventory/1`,
        body: body,
      };
    },
  );

  readonly createInventoryItem10 = requestFactory<CreateInventoryItem10Params, InventoryItem10>(
    (request: CreateInventoryItem10Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/inventory/10`,
        body: body,
      };
    },
  );

  readonly createInventoryItem2 = requestFactory<CreateInventoryItem2Params, InventoryItem2>(
    (request: CreateInventoryItem2Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/inventory/2`,
        body: body,
      };
    },
  );

  readonly createInventoryItem3 = requestFactory<CreateInventoryItem3Params, InventoryItem3>(
    (request: CreateInventoryItem3Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/inventory/3`,
        body: body,
      };
    },
  );

  readonly createInventoryItem4 = requestFactory<CreateInventoryItem4Params, InventoryItem4>(
    (request: CreateInventoryItem4Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/inventory/4`,
        body: body,
      };
    },
  );

  readonly createInventoryItem5 = requestFactory<CreateInventoryItem5Params, InventoryItem5>(
    (request: CreateInventoryItem5Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/inventory/5`,
        body: body,
      };
    },
  );

  readonly createInventoryItem6 = requestFactory<CreateInventoryItem6Params, InventoryItem6>(
    (request: CreateInventoryItem6Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/inventory/6`,
        body: body,
      };
    },
  );

  readonly createInventoryItem7 = requestFactory<CreateInventoryItem7Params, InventoryItem7>(
    (request: CreateInventoryItem7Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/inventory/7`,
        body: body,
      };
    },
  );

  readonly createInventoryItem8 = requestFactory<CreateInventoryItem8Params, InventoryItem8>(
    (request: CreateInventoryItem8Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/inventory/8`,
        body: body,
      };
    },
  );

  readonly createInventoryItem9 = requestFactory<CreateInventoryItem9Params, InventoryItem9>(
    (request: CreateInventoryItem9Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/inventory/9`,
        body: body,
      };
    },
  );

  readonly getInventoryItem1 = requestFactory<GetInventoryItem1Params, InventoryItem1List>(
    (request: GetInventoryItem1Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/inventory/1`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getInventoryItem10 = requestFactory<GetInventoryItem10Params, InventoryItem10List>(
    (request: GetInventoryItem10Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/inventory/10`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getInventoryItem2 = requestFactory<GetInventoryItem2Params, InventoryItem2List>(
    (request: GetInventoryItem2Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/inventory/2`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getInventoryItem3 = requestFactory<GetInventoryItem3Params, InventoryItem3List>(
    (request: GetInventoryItem3Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/inventory/3`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getInventoryItem4 = requestFactory<GetInventoryItem4Params, InventoryItem4List>(
    (request: GetInventoryItem4Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/inventory/4`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getInventoryItem5 = requestFactory<GetInventoryItem5Params, InventoryItem5List>(
    (request: GetInventoryItem5Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/inventory/5`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getInventoryItem6 = requestFactory<GetInventoryItem6Params, InventoryItem6List>(
    (request: GetInventoryItem6Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/inventory/6`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getInventoryItem7 = requestFactory<GetInventoryItem7Params, InventoryItem7List>(
    (request: GetInventoryItem7Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/inventory/7`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getInventoryItem8 = requestFactory<GetInventoryItem8Params, InventoryItem8List>(
    (request: GetInventoryItem8Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/inventory/8`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getInventoryItem9 = requestFactory<GetInventoryItem9Params, InventoryItem9List>(
    (request: GetInventoryItem9Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/inventory/9`,
        params: httpParams({ limit }),
      };
    },
  );
}

export interface CreateInventoryItem1Params {
  body: InventoryItem1;
}

export interface CreateInventoryItem10Params {
  body: InventoryItem10;
}

export interface CreateInventoryItem2Params {
  body: InventoryItem2;
}

export interface CreateInventoryItem3Params {
  body: InventoryItem3;
}

export interface CreateInventoryItem4Params {
  body: InventoryItem4;
}

export interface CreateInventoryItem5Params {
  body: InventoryItem5;
}

export interface CreateInventoryItem6Params {
  body: InventoryItem6;
}

export interface CreateInventoryItem7Params {
  body: InventoryItem7;
}

export interface CreateInventoryItem8Params {
  body: InventoryItem8;
}

export interface CreateInventoryItem9Params {
  body: InventoryItem9;
}

export interface GetInventoryItem1Params {
  limit?: number;
}

export interface GetInventoryItem10Params {
  limit?: number;
}

export interface GetInventoryItem2Params {
  limit?: number;
}

export interface GetInventoryItem3Params {
  limit?: number;
}

export interface GetInventoryItem4Params {
  limit?: number;
}

export interface GetInventoryItem5Params {
  limit?: number;
}

export interface GetInventoryItem6Params {
  limit?: number;
}

export interface GetInventoryItem7Params {
  limit?: number;
}

export interface GetInventoryItem8Params {
  limit?: number;
}

export interface GetInventoryItem9Params {
  limit?: number;
}
