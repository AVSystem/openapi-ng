import { Injectable } from '@angular/core';
import { httpParams, requestFactory } from '../rest.util';
import type {
  CustomersItem1,
  CustomersItem10,
  CustomersItem10List,
  CustomersItem1List,
  CustomersItem2,
  CustomersItem2List,
  CustomersItem3,
  CustomersItem3List,
  CustomersItem4,
  CustomersItem4List,
  CustomersItem5,
  CustomersItem5List,
  CustomersItem6,
  CustomersItem6List,
  CustomersItem7,
  CustomersItem7List,
  CustomersItem8,
  CustomersItem8List,
  CustomersItem9,
  CustomersItem9List,
} from '../model.generated';

@Injectable({
  providedIn: 'root',
})
export class CustomersRest {

  readonly createCustomersItem1 = requestFactory<CreateCustomersItem1Params, CustomersItem1>(
    (request: CreateCustomersItem1Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/customers/1`,
        body: body,
      };
    },
  );

  readonly createCustomersItem10 = requestFactory<CreateCustomersItem10Params, CustomersItem10>(
    (request: CreateCustomersItem10Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/customers/10`,
        body: body,
      };
    },
  );

  readonly createCustomersItem2 = requestFactory<CreateCustomersItem2Params, CustomersItem2>(
    (request: CreateCustomersItem2Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/customers/2`,
        body: body,
      };
    },
  );

  readonly createCustomersItem3 = requestFactory<CreateCustomersItem3Params, CustomersItem3>(
    (request: CreateCustomersItem3Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/customers/3`,
        body: body,
      };
    },
  );

  readonly createCustomersItem4 = requestFactory<CreateCustomersItem4Params, CustomersItem4>(
    (request: CreateCustomersItem4Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/customers/4`,
        body: body,
      };
    },
  );

  readonly createCustomersItem5 = requestFactory<CreateCustomersItem5Params, CustomersItem5>(
    (request: CreateCustomersItem5Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/customers/5`,
        body: body,
      };
    },
  );

  readonly createCustomersItem6 = requestFactory<CreateCustomersItem6Params, CustomersItem6>(
    (request: CreateCustomersItem6Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/customers/6`,
        body: body,
      };
    },
  );

  readonly createCustomersItem7 = requestFactory<CreateCustomersItem7Params, CustomersItem7>(
    (request: CreateCustomersItem7Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/customers/7`,
        body: body,
      };
    },
  );

  readonly createCustomersItem8 = requestFactory<CreateCustomersItem8Params, CustomersItem8>(
    (request: CreateCustomersItem8Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/customers/8`,
        body: body,
      };
    },
  );

  readonly createCustomersItem9 = requestFactory<CreateCustomersItem9Params, CustomersItem9>(
    (request: CreateCustomersItem9Params) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/customers/9`,
        body: body,
      };
    },
  );

  readonly getCustomersItem1 = requestFactory<GetCustomersItem1Params, CustomersItem1List>(
    (request: GetCustomersItem1Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/customers/1`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getCustomersItem10 = requestFactory<GetCustomersItem10Params, CustomersItem10List>(
    (request: GetCustomersItem10Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/customers/10`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getCustomersItem2 = requestFactory<GetCustomersItem2Params, CustomersItem2List>(
    (request: GetCustomersItem2Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/customers/2`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getCustomersItem3 = requestFactory<GetCustomersItem3Params, CustomersItem3List>(
    (request: GetCustomersItem3Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/customers/3`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getCustomersItem4 = requestFactory<GetCustomersItem4Params, CustomersItem4List>(
    (request: GetCustomersItem4Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/customers/4`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getCustomersItem5 = requestFactory<GetCustomersItem5Params, CustomersItem5List>(
    (request: GetCustomersItem5Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/customers/5`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getCustomersItem6 = requestFactory<GetCustomersItem6Params, CustomersItem6List>(
    (request: GetCustomersItem6Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/customers/6`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getCustomersItem7 = requestFactory<GetCustomersItem7Params, CustomersItem7List>(
    (request: GetCustomersItem7Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/customers/7`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getCustomersItem8 = requestFactory<GetCustomersItem8Params, CustomersItem8List>(
    (request: GetCustomersItem8Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/customers/8`,
        params: httpParams({ limit }),
      };
    },
  );

  readonly getCustomersItem9 = requestFactory<GetCustomersItem9Params, CustomersItem9List>(
    (request: GetCustomersItem9Params) => {
      const { limit } = request;
      return {
        method: 'GET',
        url: `/customers/9`,
        params: httpParams({ limit }),
      };
    },
  );
}

export interface CreateCustomersItem1Params {
  body: CustomersItem1;
}

export interface CreateCustomersItem10Params {
  body: CustomersItem10;
}

export interface CreateCustomersItem2Params {
  body: CustomersItem2;
}

export interface CreateCustomersItem3Params {
  body: CustomersItem3;
}

export interface CreateCustomersItem4Params {
  body: CustomersItem4;
}

export interface CreateCustomersItem5Params {
  body: CustomersItem5;
}

export interface CreateCustomersItem6Params {
  body: CustomersItem6;
}

export interface CreateCustomersItem7Params {
  body: CustomersItem7;
}

export interface CreateCustomersItem8Params {
  body: CustomersItem8;
}

export interface CreateCustomersItem9Params {
  body: CustomersItem9;
}

export interface GetCustomersItem1Params {
  limit?: number;
}

export interface GetCustomersItem10Params {
  limit?: number;
}

export interface GetCustomersItem2Params {
  limit?: number;
}

export interface GetCustomersItem3Params {
  limit?: number;
}

export interface GetCustomersItem4Params {
  limit?: number;
}

export interface GetCustomersItem5Params {
  limit?: number;
}

export interface GetCustomersItem6Params {
  limit?: number;
}

export interface GetCustomersItem7Params {
  limit?: number;
}

export interface GetCustomersItem8Params {
  limit?: number;
}

export interface GetCustomersItem9Params {
  limit?: number;
}
