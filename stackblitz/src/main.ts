import { provideHttpClient, withInterceptors } from '@angular/common/http';
import { bootstrapApplication } from '@angular/platform-browser';
import { App } from './app/app';
import { mockApiInterceptor } from './app/mock-api.interceptor';
import { provideOpenapiNg } from './generated/rest.util';

bootstrapApplication(App, {
  providers: [
    provideHttpClient(withInterceptors([mockApiInterceptor])),
    provideOpenapiNg({ basePath: '/api' }),
  ],
}).catch((error) => console.error(error));
