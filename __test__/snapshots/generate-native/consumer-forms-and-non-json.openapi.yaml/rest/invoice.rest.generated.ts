import { Injectable } from '@angular/core';
import { requestFactory } from '../rest.util';

@Injectable({
  providedIn: 'root',
})
export class InvoiceRest {

  readonly downloadInvoicePdf = requestFactory.blob<DownloadInvoicePdfParams>(
    (request: DownloadInvoicePdfParams) => {
      const { invoiceId } = request;
      return {
        method: 'GET',
        url: `/invoices/${encodeURIComponent(invoiceId)}/pdf`,
      };
    },
  );
}

export interface DownloadInvoicePdfParams {
  invoiceId: string;
}
