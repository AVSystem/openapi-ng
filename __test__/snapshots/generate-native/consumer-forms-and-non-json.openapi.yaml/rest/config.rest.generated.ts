import { Injectable } from '@angular/core';
import { requestFactory } from '../rest.util';

@Injectable({
  providedIn: 'root',
})
export class ConfigRest {

  readonly getRawConfig = requestFactory.zeroArg.text(
    () => ({
      method: 'GET',
      url: `/config/raw`,
    }),
  );
}
