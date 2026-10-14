import { Injectable } from '@angular/core';
import { mutationFactory } from '../rest.util';

@Injectable({
  providedIn: 'root',
})
export class SessionRest {

  readonly logout = mutationFactory.zeroArg<void>(
    () => ({
      method: 'GET',
      url: `/session/logout`,
    }),
  );
}
