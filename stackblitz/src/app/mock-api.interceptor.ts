import {
  HttpErrorResponse,
  HttpHeaders,
  HttpInterceptorFn,
  HttpResponse,
} from '@angular/common/http';
import { delay, mergeMap, of, throwError } from 'rxjs';
import type {
  EmailAvailability,
  Pet,
  UpdatePetRequest,
} from '../generated/model.generated';

// In-memory stand-in for the Petstore backend, so the demo runs with no server.
const pets: Pet[] = [
  { id: '1', name: 'Rex', status: 'available', tags: [{ id: 1, label: 'dog' }] },
  {
    id: '2',
    name: 'Misha',
    status: 'pending',
    tags: [{ id: 2, label: 'cat' }],
    nickname: 'Mishka',
  },
  { id: '3', name: 'Bubbles', status: 'available', tags: [{ id: 3, label: 'fish' }] },
  {
    id: '4',
    name: 'Kiwi',
    status: 'sold',
    tags: [{ id: 4, label: 'bird' }],
    nickname: null,
  },
];

const takenEmails = new Set(['taken@example.com', 'admin@example.com']);

const LATENCY_MS = 500;

export const mockApiInterceptor: HttpInterceptorFn = (req, next) => {
  const url = new URL(req.urlWithParams, 'http://localhost');
  if (!url.pathname.startsWith('/api/')) return next(req);

  const route = `${req.method} ${url.pathname.slice('/api'.length)}`;
  console.info('[mock-api]', route, url.search, req.body ?? '');
  return of(null).pipe(
    delay(LATENCY_MS),
    mergeMap(() => {
      const match = /^(GET|POST) \/pets\/([^/]+)$/.exec(route);
      if (route === 'GET /pets') return ok(listPets(url.searchParams.get('status')));
      if (match) {
        const pet = pets.find(candidate => candidate.id === match[2]);
        if (!pet) return fail(404, `No pet with id ${match[2]}`);
        return ok(
          match[1] === 'GET' ? pet : updatePet(pet, req.body as UpdatePetRequest),
        );
      }
      if (route === 'GET /accounts/check-email') {
        return ok(checkEmail(url.searchParams.get('email') ?? ''));
      }
      return fail(404, `Unhandled route ${route}`);
    }),
  );
};

function listPets(status: string | null): Pet[] {
  return status ? pets.filter(pet => pet.status === status) : pets;
}

function updatePet(pet: Pet, body: UpdatePetRequest): Pet {
  pet.status = body.status;
  if (body.nickname !== undefined) pet.nickname = body.nickname;
  return pet;
}

function checkEmail(email: string): EmailAvailability {
  const available = !takenEmails.has(email.toLowerCase());
  return available
    ? { available }
    : { available, suggestion: email.replace('@', '.2026@') };
}

function ok<T>(body: T) {
  return of(
    new HttpResponse({
      status: 200,
      body: structuredClone(body),
      headers: new HttpHeaders({ 'x-mock-latency': `${LATENCY_MS}ms` }),
    }),
  );
}

function fail(status: number, message: string) {
  return throwError(
    () => new HttpErrorResponse({ status, statusText: message, error: { message } }),
  );
}
