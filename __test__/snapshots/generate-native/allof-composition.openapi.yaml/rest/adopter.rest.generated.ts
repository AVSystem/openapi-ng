import { Injectable } from '@angular/core';
import { requestFactory } from '../rest.util';
import type { AdopterProfile } from '../model.generated';

@Injectable({
  providedIn: 'root',
})
export class AdopterRest {

  readonly getAdopterProfile = requestFactory<GetAdopterProfileParams, AdopterProfile>(
    (request: GetAdopterProfileParams) => {
      const { adopterId } = request;
      return {
        method: 'GET',
        url: `/adopters/${encodeURIComponent(adopterId)}`,
      };
    },
  );
}

export interface GetAdopterProfileParams {
  adopterId: string;
}
