import { Injectable } from '@angular/core';
import { requestFactory } from '../rest.util';

@Injectable({
  providedIn: 'root',
})
export class BlobRest {

  readonly getBlob = requestFactory.blob<GetBlobParams>(
    (request: GetBlobParams) => {
      const { id } = request;
      return {
        method: 'GET',
        url: `/blob/${encodeURIComponent(id)}`,
      };
    },
  );
}

export interface GetBlobParams {
  id: string;
}
