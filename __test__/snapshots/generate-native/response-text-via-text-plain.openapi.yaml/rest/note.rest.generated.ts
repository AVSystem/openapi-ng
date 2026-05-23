import { Injectable } from '@angular/core';
import { requestFactory } from '../rest.util';

@Injectable({
  providedIn: 'root',
})
export class NoteRest {

  readonly getNote = requestFactory.text<GetNoteParams>(
    (request: GetNoteParams) => {
      const { id } = request;
      return {
        method: 'GET',
        url: `/notes/${encodeURIComponent(id)}`,
      };
    },
  );
}

export interface GetNoteParams {
  id: string;
}
