import { Component, signal } from '@angular/core';
import type { PetId } from '../generated/model.generated';
import { PetDetail } from './pet-detail';
import { PetList } from './pet-list';
import { SignupForm } from './signup-form';

@Component({
  selector: 'app-root',
  imports: [PetList, PetDetail, SignupForm],
  template: `
    <h1>openapi-ng <small>generated Angular client, mock backend via HttpInterceptor</small></h1>
    <p class="muted">
      Everything under <code>src/generated/</code> came out of <code>openapi-ng generate</code> run on
      <code>petstore.openapi.yaml</code>. The components in <code>src/app/</code> consume it three ways:
      <code>.resource()</code>, <code>.observable()</code>, and <code>.request()</code>.
    </p>
    <div class="grid">
      <section><app-pet-list #list (selected)="selectedId.set($event)" /></section>
      <section><app-pet-detail [petId]="selectedId()" (updated)="list.pets.reload()" /></section>
      <section><app-signup-form /></section>
    </div>
  `,
})
export class App {
  protected readonly selectedId = signal<PetId | undefined>(undefined);
}
