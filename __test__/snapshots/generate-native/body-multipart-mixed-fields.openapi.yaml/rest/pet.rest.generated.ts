import { Injectable } from '@angular/core';
import { requestFactory } from '../rest.util';

@Injectable({
  providedIn: 'root',
})
export class PetRest {

  readonly updatePetAvatar = requestFactory<UpdatePetAvatarParams, {
    id?: string;
  }>(
    (request: UpdatePetAvatarParams) => {
      const { petId, avatar, galleries, nickname, status, tagIds } = request;
      return {
        method: 'POST',
        url: `/pets/${encodeURIComponent(petId)}/avatar`,
        body: ((): FormData => {
          const fd = new FormData();
          fd.append('avatar', avatar);
          for (const v of galleries) fd.append('galleries', v);
          if (nickname !== undefined) fd.append('nickname', String(nickname));
          fd.append('status', String(status));
          if (tagIds !== undefined) for (const v of tagIds) fd.append('tagIds', String(v));
          return fd;
        })(),
      };
    },
  );
}

export interface UpdatePetAvatarParams {
  petId: string;
  avatar: Blob | File;
  galleries: (Blob | File)[];
  nickname?: string;
  status: string;
  tagIds?: number[];
}
