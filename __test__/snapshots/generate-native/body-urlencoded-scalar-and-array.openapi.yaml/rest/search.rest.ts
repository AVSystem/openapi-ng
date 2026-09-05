import { Injectable } from '@angular/core';
import { requestFactory } from '../rest.util';

@Injectable({
  providedIn: 'root',
})
export class SearchRest {

  readonly submitSearch = requestFactory<SubmitSearchParams, {
    count?: number;
  }>(
    (request: SubmitSearchParams) => {
      const { query, tagIds } = request;
      return {
        method: 'POST',
        url: `/search`,
        body: ((): URLSearchParams => {
          const params = new URLSearchParams();
          params.append('query', String(query));
          if (tagIds !== undefined) for (const v of tagIds) params.append('tagIds', String(v));
          return params;
        })(),
      };
    },
  );
}

export interface SubmitSearchParams {
  query: string;
  tagIds?: number[];
}
