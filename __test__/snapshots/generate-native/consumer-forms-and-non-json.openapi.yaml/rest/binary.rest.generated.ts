import { Injectable } from '@angular/core';
import { requestFactory } from '../rest.util';

@Injectable({
  providedIn: 'root',
})
export class BinaryRest {

  readonly fetchBlob = requestFactory.zeroArg.blob(
    () => ({
      method: 'GET',
      url: `/binary/fetch`,
    }),
  );
}
