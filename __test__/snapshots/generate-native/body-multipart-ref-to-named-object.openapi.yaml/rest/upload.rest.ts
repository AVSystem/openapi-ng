import { Injectable } from '@angular/core';
import { mutationFactory } from '../rest.util';

@Injectable({
  providedIn: 'root',
})
export class UploadRest {

  readonly createUpload = mutationFactory<CreateUploadParams, {
    id?: string;
  }>(
    (request: CreateUploadParams) => {
      const { description, title } = request;
      return {
        method: 'POST',
        url: `/uploads`,
        body: ((): FormData => {
          const fd = new FormData();
          if (description !== undefined) fd.append('description', String(description));
          fd.append('title', String(title));
          return fd;
        })(),
      };
    },
  );
}

export interface CreateUploadParams {
  description?: string;
  title: string;
}
