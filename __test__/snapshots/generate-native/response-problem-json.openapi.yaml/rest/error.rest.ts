import { Injectable } from '@angular/core';
import { requestFactory } from '../rest.util';

@Injectable({
  providedIn: 'root',
})
export class ErrorRest {

  readonly getError = requestFactory<GetErrorParams, {
    detail?: string;
  }>(
    (request: GetErrorParams) => {
      const { id } = request;
      return {
        method: 'GET',
        url: `/errors/${encodeURIComponent(id)}`,
      };
    },
  );
}

export interface GetErrorParams {
  id: string;
}
