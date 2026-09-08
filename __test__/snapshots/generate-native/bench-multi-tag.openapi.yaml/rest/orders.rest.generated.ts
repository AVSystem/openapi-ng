import { Injectable } from '@angular/core';
import { httpParams, requestFactory } from '../rest.util';
import type {
  OrdersItem1,
  OrdersItem10,
  OrdersItem10List,
  OrdersItem1List,
  OrdersItem2,
  OrdersItem2List,
  OrdersItem3,
  OrdersItem3List,
  OrdersItem4,
  OrdersItem4List,
  OrdersItem5,
  OrdersItem5List,
  OrdersItem6,
  OrdersItem6List,
  OrdersItem7,
  OrdersItem7List,
  OrdersItem8,
  OrdersItem8List,
  OrdersItem9,
  OrdersItem9List,
} from '../model.generated';

@Injectable({
  providedIn: 'root',
})
export class OrdersRest {

  readonly createOrdersItem1 = requestFactory<CreateOrdersItem1Params, OrdersItem1>(
    (request: CreateOrdersItem1Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/orders/1`,
        body: body,
      };
    },
  );

  readonly createOrdersItem10 = requestFactory<CreateOrdersItem10Params, OrdersItem10>(
    (request: CreateOrdersItem10Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/orders/10`,
        body: body,
      };
    },
  );

  readonly createOrdersItem2 = requestFactory<CreateOrdersItem2Params, OrdersItem2>(
    (request: CreateOrdersItem2Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/orders/2`,
        body: body,
      };
    },
  );

  readonly createOrdersItem3 = requestFactory<CreateOrdersItem3Params, OrdersItem3>(
    (request: CreateOrdersItem3Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/orders/3`,
        body: body,
      };
    },
  );

  readonly createOrdersItem4 = requestFactory<CreateOrdersItem4Params, OrdersItem4>(
    (request: CreateOrdersItem4Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/orders/4`,
        body: body,
      };
    },
  );

  readonly createOrdersItem5 = requestFactory<CreateOrdersItem5Params, OrdersItem5>(
    (request: CreateOrdersItem5Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/orders/5`,
        body: body,
      };
    },
  );

  readonly createOrdersItem6 = requestFactory<CreateOrdersItem6Params, OrdersItem6>(
    (request: CreateOrdersItem6Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/orders/6`,
        body: body,
      };
    },
  );

  readonly createOrdersItem7 = requestFactory<CreateOrdersItem7Params, OrdersItem7>(
    (request: CreateOrdersItem7Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/orders/7`,
        body: body,
      };
    },
  );

  readonly createOrdersItem8 = requestFactory<CreateOrdersItem8Params, OrdersItem8>(
    (request: CreateOrdersItem8Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/orders/8`,
        body: body,
      };
    },
  );

  readonly createOrdersItem9 = requestFactory<CreateOrdersItem9Params, OrdersItem9>(
    (request: CreateOrdersItem9Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/orders/9`,
        body: body,
      };
    },
  );

  readonly getOrdersItem1 = requestFactory<GetOrdersItem1Params, OrdersItem1List>(
    (request: GetOrdersItem1Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/orders/1`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getOrdersItem10 = requestFactory<GetOrdersItem10Params, OrdersItem10List>(
    (request: GetOrdersItem10Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/orders/10`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getOrdersItem2 = requestFactory<GetOrdersItem2Params, OrdersItem2List>(
    (request: GetOrdersItem2Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/orders/2`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getOrdersItem3 = requestFactory<GetOrdersItem3Params, OrdersItem3List>(
    (request: GetOrdersItem3Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/orders/3`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getOrdersItem4 = requestFactory<GetOrdersItem4Params, OrdersItem4List>(
    (request: GetOrdersItem4Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/orders/4`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getOrdersItem5 = requestFactory<GetOrdersItem5Params, OrdersItem5List>(
    (request: GetOrdersItem5Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/orders/5`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getOrdersItem6 = requestFactory<GetOrdersItem6Params, OrdersItem6List>(
    (request: GetOrdersItem6Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/orders/6`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getOrdersItem7 = requestFactory<GetOrdersItem7Params, OrdersItem7List>(
    (request: GetOrdersItem7Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/orders/7`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getOrdersItem8 = requestFactory<GetOrdersItem8Params, OrdersItem8List>(
    (request: GetOrdersItem8Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/orders/8`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getOrdersItem9 = requestFactory<GetOrdersItem9Params, OrdersItem9List>(
    (request: GetOrdersItem9Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/orders/9`,
        params: httpParams({ limit }),
      };
    },
  );
}

export interface CreateOrdersItem1Params {
  body: OrdersItem1;
}

export interface CreateOrdersItem10Params {
  body: OrdersItem10;
}

export interface CreateOrdersItem2Params {
  body: OrdersItem2;
}

export interface CreateOrdersItem3Params {
  body: OrdersItem3;
}

export interface CreateOrdersItem4Params {
  body: OrdersItem4;
}

export interface CreateOrdersItem5Params {
  body: OrdersItem5;
}

export interface CreateOrdersItem6Params {
  body: OrdersItem6;
}

export interface CreateOrdersItem7Params {
  body: OrdersItem7;
}

export interface CreateOrdersItem8Params {
  body: OrdersItem8;
}

export interface CreateOrdersItem9Params {
  body: OrdersItem9;
}

export interface GetOrdersItem1Params {
  limit?: number;
}

export interface GetOrdersItem10Params {
  limit?: number;
}

export interface GetOrdersItem2Params {
  limit?: number;
}

export interface GetOrdersItem3Params {
  limit?: number;
}

export interface GetOrdersItem4Params {
  limit?: number;
}

export interface GetOrdersItem5Params {
  limit?: number;
}

export interface GetOrdersItem6Params {
  limit?: number;
}

export interface GetOrdersItem7Params {
  limit?: number;
}

export interface GetOrdersItem8Params {
  limit?: number;
}

export interface GetOrdersItem9Params {
  limit?: number;
}
