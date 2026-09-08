import { Injectable } from '@angular/core';
import { requestFactory } from '../rest.util';

@Injectable({
  providedIn: 'root',
})
export class PostRest {

  readonly postsCreate = requestFactory<PostsCreateParams, void>(
    (request: PostsCreateParams) => {
      const { body } = request;
      return {
        method: 'POST',
        url: `/posts`,
        body: body,
      };
    },
  );

  readonly postsListAll = requestFactory.zeroArg<string[]>(
    () => ({
      method: 'GET',
      url: `/posts`,
    }),
  );
}

export interface PostsCreateParams {
  body: string;
}
