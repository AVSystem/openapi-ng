import { JsonPipe } from '@angular/common';
import { Component, inject, signal } from '@angular/core';
import { email, form, FormField, required } from '@angular/forms/signals';
import { AccountRest } from '../generated/rest/account.rest.generated';
import { validateRest } from '../generated/rest.validate';

interface Signup {
  name: string;
  email: string;
}

@Component({
  selector: 'app-signup-form',
  imports: [FormField, JsonPipe],
  template: `
    <h2>Signal forms: validateRest()</h2>
    <p class="muted">
      The async validator reuses the generated
      <code>AccountRest.checkEmail</code> operation, so request and response stay typed.
      Try <code>taken@example.com</code>.
    </p>

    <form (submit)="onSubmit($event)">
      <label for="name">Name</label>
      <input id="name" [formField]="signup.name" />

      <label for="email">Email</label>
      <input id="email" type="email" [formField]="signup.email" />
      @if (signup.email().pending()) {
        <p class="muted">checking availability…</p>
      } @else if (signup.email().dirty() || signup.email().touched()) {
        @for (error of signup.email().errors(); track error.kind) {
          <p class="error">{{ error.message ?? error.kind }}</p>
        }
        @if (signup.email().valid()) {
          <p class="ok">available</p>
        }
      }

      <p><button type="submit" [disabled]="!signup().valid()">Sign up</button></p>
    </form>

    @if (submitted()) {
      <pre>{{ model() | json }}</pre>
    }
  `,
})
export class SignupForm {
  readonly #accountRest = inject(AccountRest);

  protected readonly model = signal<Signup>({ name: '', email: '' });
  protected readonly submitted = signal(false);

  protected readonly signup = form(this.model, path => {
    required(path.name);
    required(path.email);
    email(path.email);
    validateRest(path.email, this.#accountRest.checkEmail, {
      // typed as CheckEmailParams; a typo here is a compile error
      request: ctx => ({ email: ctx.value() }),
      debounce: 300,
      when: ctx => ctx.value().includes('@'),
      onSuccess: result =>
        result.available
          ? undefined
          : {
              kind: 'email-taken',
              message: `Already taken. How about ${result.suggestion}?`,
            },
      onError: () => ({ kind: 'check-failed', message: 'Could not verify the address.' }),
    });
  });

  protected onSubmit(event: Event) {
    event.preventDefault();
    this.submitted.set(true);
  }
}
