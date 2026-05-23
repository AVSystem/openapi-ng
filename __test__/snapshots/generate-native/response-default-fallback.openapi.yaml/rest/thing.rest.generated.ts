import { Injectable } from '@angular/core';
import { requestFactory } from '../rest.util';
import type { Thing } from '../model.generated';

@Injectable({
  providedIn: 'root',
})
export class ThingRest {

  readonly getThing = requestFactory.zeroArg<Thing>(
    () => ({
      method: 'GET',
      url: `/thing`,
    }),
  );
}
