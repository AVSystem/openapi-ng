import { defineMutation } from '../../rest.util';

export const renderReport = /* @__PURE__ */ defineMutation.zeroArg.text(
  'renderReport',
  () => ({
    method: 'POST',
    url: `/reports/render`,
  }),
);
