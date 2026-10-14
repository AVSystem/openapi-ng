// Must not compile: an operation the `resources` rules leave out is typed
// without `.resource()`. tsc must reject each line below with TS2339.
import type { PetRest } from '../../generated/rest/pet.rest';
import type { SessionRest } from '../../generated/rest/session.rest';

declare const service: PetRest;
declare const session: SessionRest;

// A POST the default rules leave out.
export const createPet = service.createPet.resource(() => ({ body: { name: 'rex' } }));
// A DELETE with a `void` response.
export const deletePet = service.deletePet.resource(() => ({ petId: 'p-1' }));
// A zero-arg GET opted out by `x-openapi-ng-resource: false`.
export const logout = session.logout.resource();
