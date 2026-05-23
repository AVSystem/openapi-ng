import { Injectable } from '@angular/core';
import { requestFactory } from '../rest.util';

@Injectable({
  providedIn: 'root',
})
export class UtilRest {

  readonly deletePing = requestFactory.zeroArg<void>(
    () => ({
      method: 'DELETE',
      url: `/ping`,
    }),
  );
}
