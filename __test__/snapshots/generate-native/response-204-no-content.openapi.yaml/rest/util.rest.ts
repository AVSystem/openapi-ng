import { Injectable } from '@angular/core';
import { mutationFactory } from '../rest.util';

@Injectable({
  providedIn: 'root',
})
export class UtilRest {

  readonly deletePing = mutationFactory.zeroArg<void>(
    () => ({
      method: 'DELETE',
      url: `/ping`,
    }),
  );
}
