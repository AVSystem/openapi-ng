import { Injectable } from '@angular/core';
import { httpParams, requestFactory } from '../rest.util';
import type {
  ShippingItem1,
  ShippingItem10,
  ShippingItem10List,
  ShippingItem1List,
  ShippingItem2,
  ShippingItem2List,
  ShippingItem3,
  ShippingItem3List,
  ShippingItem4,
  ShippingItem4List,
  ShippingItem5,
  ShippingItem5List,
  ShippingItem6,
  ShippingItem6List,
  ShippingItem7,
  ShippingItem7List,
  ShippingItem8,
  ShippingItem8List,
  ShippingItem9,
  ShippingItem9List,
} from '../model.generated';

@Injectable({
  providedIn: 'root',
})
export class ShippingRest {

  readonly createShippingItem1 = requestFactory<CreateShippingItem1Params, ShippingItem1>(
    (request: CreateShippingItem1Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/shipping/1`,
        body: body,
      };
    },
  );

  readonly createShippingItem10 = requestFactory<CreateShippingItem10Params, ShippingItem10>(
    (request: CreateShippingItem10Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/shipping/10`,
        body: body,
      };
    },
  );

  readonly createShippingItem2 = requestFactory<CreateShippingItem2Params, ShippingItem2>(
    (request: CreateShippingItem2Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/shipping/2`,
        body: body,
      };
    },
  );

  readonly createShippingItem3 = requestFactory<CreateShippingItem3Params, ShippingItem3>(
    (request: CreateShippingItem3Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/shipping/3`,
        body: body,
      };
    },
  );

  readonly createShippingItem4 = requestFactory<CreateShippingItem4Params, ShippingItem4>(
    (request: CreateShippingItem4Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/shipping/4`,
        body: body,
      };
    },
  );

  readonly createShippingItem5 = requestFactory<CreateShippingItem5Params, ShippingItem5>(
    (request: CreateShippingItem5Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/shipping/5`,
        body: body,
      };
    },
  );

  readonly createShippingItem6 = requestFactory<CreateShippingItem6Params, ShippingItem6>(
    (request: CreateShippingItem6Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/shipping/6`,
        body: body,
      };
    },
  );

  readonly createShippingItem7 = requestFactory<CreateShippingItem7Params, ShippingItem7>(
    (request: CreateShippingItem7Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/shipping/7`,
        body: body,
      };
    },
  );

  readonly createShippingItem8 = requestFactory<CreateShippingItem8Params, ShippingItem8>(
    (request: CreateShippingItem8Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/shipping/8`,
        body: body,
      };
    },
  );

  readonly createShippingItem9 = requestFactory<CreateShippingItem9Params, ShippingItem9>(
    (request: CreateShippingItem9Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/shipping/9`,
        body: body,
      };
    },
  );

  readonly getShippingItem1 = requestFactory<GetShippingItem1Params, ShippingItem1List>(
    (request: GetShippingItem1Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/shipping/1`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getShippingItem10 = requestFactory<GetShippingItem10Params, ShippingItem10List>(
    (request: GetShippingItem10Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/shipping/10`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getShippingItem2 = requestFactory<GetShippingItem2Params, ShippingItem2List>(
    (request: GetShippingItem2Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/shipping/2`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getShippingItem3 = requestFactory<GetShippingItem3Params, ShippingItem3List>(
    (request: GetShippingItem3Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/shipping/3`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getShippingItem4 = requestFactory<GetShippingItem4Params, ShippingItem4List>(
    (request: GetShippingItem4Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/shipping/4`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getShippingItem5 = requestFactory<GetShippingItem5Params, ShippingItem5List>(
    (request: GetShippingItem5Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/shipping/5`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getShippingItem6 = requestFactory<GetShippingItem6Params, ShippingItem6List>(
    (request: GetShippingItem6Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/shipping/6`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getShippingItem7 = requestFactory<GetShippingItem7Params, ShippingItem7List>(
    (request: GetShippingItem7Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/shipping/7`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getShippingItem8 = requestFactory<GetShippingItem8Params, ShippingItem8List>(
    (request: GetShippingItem8Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/shipping/8`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getShippingItem9 = requestFactory<GetShippingItem9Params, ShippingItem9List>(
    (request: GetShippingItem9Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/shipping/9`,
        params: httpParams({ limit }),
      };
    },
  );
}

export interface CreateShippingItem1Params {
  body: ShippingItem1;
}

export interface CreateShippingItem10Params {
  body: ShippingItem10;
}

export interface CreateShippingItem2Params {
  body: ShippingItem2;
}

export interface CreateShippingItem3Params {
  body: ShippingItem3;
}

export interface CreateShippingItem4Params {
  body: ShippingItem4;
}

export interface CreateShippingItem5Params {
  body: ShippingItem5;
}

export interface CreateShippingItem6Params {
  body: ShippingItem6;
}

export interface CreateShippingItem7Params {
  body: ShippingItem7;
}

export interface CreateShippingItem8Params {
  body: ShippingItem8;
}

export interface CreateShippingItem9Params {
  body: ShippingItem9;
}

export interface GetShippingItem1Params {
  limit?: number;
}

export interface GetShippingItem10Params {
  limit?: number;
}

export interface GetShippingItem2Params {
  limit?: number;
}

export interface GetShippingItem3Params {
  limit?: number;
}

export interface GetShippingItem4Params {
  limit?: number;
}

export interface GetShippingItem5Params {
  limit?: number;
}

export interface GetShippingItem6Params {
  limit?: number;
}

export interface GetShippingItem7Params {
  limit?: number;
}

export interface GetShippingItem8Params {
  limit?: number;
}

export interface GetShippingItem9Params {
  limit?: number;
}
