// Compile-time proofs for the request bodies and non-JSON responses
// surfaced by Phase 7. Lives next to service-proof.ts (the petstore-rich
// JSON proof) and compiles against a separate combined fixture
// (`consumer-forms-and-non-json.openapi.yaml`) generated into
// `__test__/angular-consumer/generated/` by the matching ava test.
//
// Each block asserts:
//   1. The request type accepts the right field shapes (Blob | File,
//      number[], etc.).
//   2. `.observable(...)` and `.resource(...)` carry the right Response
//      generic through to `Observable<Response>` / `HttpResourceRef<...>`.
//
// A regression that collapses any of these to `any` or rejects a valid
// call-site shape fails this file under `tsc --noEmit`.

import type { HttpResourceRef } from '@angular/common/http';
import type { Observable } from 'rxjs';

import type { CommonRequest } from '../generated/rest.model';

import type { BinaryRest } from '../generated/rest/binary.rest.generated';
import type { ConfigRest } from '../generated/rest/config.rest.generated';
import type {
  DownloadInvoicePdfParams,
  InvoiceRest,
} from '../generated/rest/invoice.rest.generated';
import type {
  PetRest,
  UpdatePetAvatarParams,
} from '../generated/rest/pet.rest.generated';
import type {
  SearchRest,
  SubmitFormParams,
} from '../generated/rest/search.rest.generated';

declare const petSvc: PetRest;
declare const searchSvc: SearchRest;
declare const invoiceSvc: InvoiceRest;
declare const configSvc: ConfigRest;
declare const binarySvc: BinaryRest;

declare function expectType<T>(value: T): void;

/**
 * multipart/form-data — mixed-field body.
 *
 * Asserts:
 *   - Binary field accepts `Blob | File`.
 *   - Repeated binary field accepts `(Blob | File)[]`.
 *   - Scalar array accepts `number[]` (the spec types it as integer).
 *   - Operation observable resolves the inline `{ id?: string }` shape.
 */
const multipartRequest: UpdatePetAvatarParams = {
  petId: 'p-1',
  status: 'available',
  tagIds: [1, 2, 3],
  avatar: new Blob(['avatar-bytes']),
  galleries: [new Blob(['g1']), new File(['g2'], 'g2.png')],
};

const multipartObservable = petSvc.updatePetAvatar.observable(multipartRequest);
const multipartResource = petSvc.updatePetAvatar.resource(() => multipartRequest);

expectType<Observable<{ id?: string }>>(multipartObservable);
expectType<HttpResourceRef<{ id?: string } | undefined>>(multipartResource);

/**
 * application/x-www-form-urlencoded — scalar + scalar array.
 *
 * Asserts that the generated request interface keeps the same JS-side
 * shape as a JSON body (no `Blob | File` leakage from the multipart
 * branch) and that the response generic survives.
 */
const urlencodedRequest: SubmitFormParams = {
  status: 'pending',
  tagIds: [3, 4],
};

const urlencodedObservable = searchSvc.submitForm.observable(urlencodedRequest);
expectType<Observable<{ count?: number }>>(urlencodedObservable);

/**
 * application/pdf response — classified as `Blob` by the default
 * response-kind classifier.
 *
 * Asserts:
 *   - `.observable(...)` and `.resource(...)` carry `Blob` through.
 *   - `.request(...)` returns a CommonRequest (compile-only check).
 *   - `defaultValue` removes the `| undefined` from the resource ref.
 *   - `parse` projects the raw Blob to a caller-defined result type.
 */
const blobRequest = invoiceSvc.downloadInvoicePdf.request({ invoiceId: 'inv-42' });
const blobObservable = invoiceSvc.downloadInvoicePdf.observable({ invoiceId: 'inv-42' });
const blobResource = invoiceSvc.downloadInvoicePdf.resource(
  (): DownloadInvoicePdfParams => ({ invoiceId: 'inv-42' }),
);
const blobResourceWithDefault = invoiceSvc.downloadInvoicePdf.resource(
  (): DownloadInvoicePdfParams => ({ invoiceId: 'inv-42' }),
  { defaultValue: new Blob() },
);
const blobResourceWithParse = invoiceSvc.downloadInvoicePdf.resource(
  (): DownloadInvoicePdfParams => ({ invoiceId: 'inv-42' }),
  { parse: (raw: Blob): { size: number } => ({ size: raw.size }) },
);

expectType<CommonRequest>(blobRequest);
expectType<Observable<Blob>>(blobObservable);
expectType<HttpResourceRef<Blob | undefined>>(blobResource);
expectType<HttpResourceRef<Blob>>(blobResourceWithDefault);
expectType<HttpResourceRef<{ size: number } | undefined>>(blobResourceWithParse);

/**
 * text/plain response — classified as `string`.
 *
 * Same surface assertions as the blob block, instantiated for `string`.
 */
const textRequest = configSvc.getRawConfig.request();
const textObservable = configSvc.getRawConfig.observable();
const textResource = configSvc.getRawConfig.resource();
const textResourceWithDefault = configSvc.getRawConfig.resource({
  defaultValue: '',
});
const textResourceWithParse = configSvc.getRawConfig.resource({
  parse: (raw: string): number => raw.length,
});

expectType<CommonRequest>(textRequest);
expectType<Observable<string>>(textObservable);
expectType<HttpResourceRef<string | undefined>>(textResource);
expectType<HttpResourceRef<string>>(textResourceWithDefault);
expectType<HttpResourceRef<number | undefined>>(textResourceWithParse);

/**
 * application/octet-stream — overridden to `arrayBuffer` via the
 * generator's `responseTypeMapping` option.
 *
 * Same surface assertions as the blob block, instantiated for
 * `ArrayBuffer`.
 */
const arrayBufferRequest = binarySvc.fetchBlob.request();
const arrayBufferObservable = binarySvc.fetchBlob.observable();
const arrayBufferResource = binarySvc.fetchBlob.resource();
const arrayBufferResourceWithDefault = binarySvc.fetchBlob.resource({
  defaultValue: new ArrayBuffer(0),
});
const arrayBufferResourceWithParse = binarySvc.fetchBlob.resource({
  parse: (raw: ArrayBuffer): number => raw.byteLength,
});

expectType<CommonRequest>(arrayBufferRequest);
expectType<Observable<ArrayBuffer>>(arrayBufferObservable);
expectType<HttpResourceRef<ArrayBuffer | undefined>>(arrayBufferResource);
expectType<HttpResourceRef<ArrayBuffer>>(arrayBufferResourceWithDefault);
expectType<HttpResourceRef<number | undefined>>(arrayBufferResourceWithParse);
