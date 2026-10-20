import { Injectable } from '@angular/core';
import * as ops from './session';

@Injectable({
  providedIn: 'root',
})
export class SessionRest {
  readonly logout = ops.logout.withInjector();
}
