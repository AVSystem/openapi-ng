import { Injectable } from '@angular/core';
import { requestFactory } from '../rest.util';

@Injectable({
  providedIn: 'root',
})
export class ReportRest {

  readonly getReport = requestFactory.blob<GetReportParams>(
    (request: GetReportParams) => {
      const { id } = request;
      return {
        method: 'GET',
        url: `/reports/${encodeURIComponent(id)}`,
      };
    },
  );
}

export interface GetReportParams {
  id: string;
}
