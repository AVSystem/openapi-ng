import { defineMutation } from '../../rest.util';

export const logout = /* @__PURE__ */ defineMutation.zeroArg<void>(
  'logout',
  () => ({
    method: 'GET',
    url: `/session/logout`,
  }),
);
