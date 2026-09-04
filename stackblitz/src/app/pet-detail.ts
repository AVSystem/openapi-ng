import { JsonPipe } from '@angular/common';
import { Component, computed, inject, input, output, signal } from '@angular/core';
import type { Pet, PetId, PetStatus } from '../generated/model.generated';
import { PetRest } from '../generated/rest/pet.rest.generated';

@Component({
  selector: 'app-pet-detail',
  imports: [JsonPipe],
  template: `
    <h2>getPet.resource() and updatePet.observable()</h2>

    @if (!petId()) {
      <p class="muted">Pick a pet on the left.</p>
    } @else if (pet.isLoading()) {
      <p class="muted">loading…</p>
    } @else if (pet.error()) {
      <p class="error">{{ pet.error() }}</p>
    } @else if (pet.value(); as current) {
      <p>
        <strong>{{ current.name }}</strong>
        <span class="muted">#{{ current.id }}, {{ current.status }}</span>
        @if (current.nickname) {
          <span class="muted">aka {{ current.nickname }}</span>
        }
      </p>
      <p>
        @for (option of statuses; track option) {
          <button
            [disabled]="saving() || current.status === option"
            (click)="setStatus(option)"
          >
            mark {{ option }}
          </button>
        }
      </p>
    }

    @if (rawRequest(); as raw) {
      <p class="muted">
        .request() returns the plain request, if you want to send it yourself:
      </p>
      <pre>{{ raw | json }}</pre>
    }
  `,
})
export class PetDetail {
  readonly #petRest = inject(PetRest);

  readonly petId = input<PetId>();
  readonly updated = output<Pet>();

  protected readonly statuses: PetStatus[] = ['available', 'pending', 'sold'];
  protected readonly saving = signal(false);

  // Returning undefined from the callback keeps the resource idle until a pet is picked.
  readonly pet = this.#petRest.getPet.resource(() => {
    const petId = this.petId();
    return petId ? { petId } : undefined;
  });

  protected readonly rawRequest = computed(() => {
    const petId = this.petId();
    return petId ? this.#petRest.getPet.request({ petId }) : undefined;
  });

  protected setStatus(status: PetStatus) {
    const current = this.pet.value();
    if (!current) return;
    this.saving.set(true);
    this.#petRest.updatePet
      .observable({ petId: current.id, body: { status } }, { observe: 'response' })
      .subscribe({
        next: response => {
          const saved = response.body!;
          this.pet.set(saved);
          this.updated.emit(saved);
        },
        error: () => this.saving.set(false),
        complete: () => this.saving.set(false),
      });
  }
}
