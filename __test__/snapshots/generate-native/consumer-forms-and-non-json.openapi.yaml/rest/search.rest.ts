import { Injectable } from '@angular/core';
import { requestFactory } from '../rest.util';

@Injectable({
  providedIn: 'root',
})
export class SearchRest {

  readonly submitForm = requestFactory<SubmitFormParams, {
    count?: number;
  }>(
    (request: SubmitFormParams) => {
      const { status, tagIds } = request;
      return {
        method: 'POST',
        url: `/search`,
        body: ((): URLSearchParams => {
          const params = new URLSearchParams();
          params.append('status', String(status));
          for (const v of tagIds) params.append('tagIds', String(v));
          return params;
        })(),
      };
    },
  );
}

export interface SubmitFormParams {
  status: string;
  tagIds: number[];
}
