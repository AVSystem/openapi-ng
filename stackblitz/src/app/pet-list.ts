import { Component, inject, output, signal } from '@angular/core';
import type { PetId, PetStatus } from '../generated/model.generated';
import { PetRest } from '../generated/rest/pet.rest.generated';

@Component({
  selector: 'app-pet-list',
  template: `
    <h2>listPets.resource()</h2>
    <label for="status">Status filter</label>
    <select id="status" [value]="status() ?? ''" (change)="onStatusChange($event)">
      <option value="">all</option>
      @for (option of statuses; track option) {
        <option [value]="option">{{ option }}</option>
      }
    </select>

    <p class="muted">
      @if (pets.isLoading()) {
        loading…
      } @else {
        {{ pets.value().length }} pets, HTTP {{ pets.statusCode() }}, server latency
        {{ pets.headers()?.get('x-mock-latency') }}
      }
      <button class="link" (click)="pets.reload()">reload</button>
    </p>

    <ul>
      @for (pet of pets.value(); track pet.id) {
        <li>
          <button class="link" (click)="selected.emit(pet.id)">{{ pet.name }}</button>
          <span class="muted">{{ pet.status }}</span>
        </li>
      }
    </ul>
  `,
})
export class PetList {
  readonly #petRest = inject(PetRest);

  readonly selected = output<PetId>();

  protected readonly statuses: PetStatus[] = ['available', 'pending', 'sold'];
  protected readonly status = signal<PetStatus | undefined>(undefined);

  // Re-fetches whenever the filter signal changes. Typed HttpResourceRef<PetList>.
  readonly pets = this.#petRest.listPets.resource(() => ({ status: this.status() }), {
    defaultValue: [],
  });

  protected onStatusChange(event: Event) {
    const value = (event.target as HTMLSelectElement).value as PetStatus | '';
    this.status.set(value || undefined);
  }
}
