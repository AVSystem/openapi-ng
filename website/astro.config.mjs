import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

export default defineConfig({
  site: 'https://docs.openapi-ng.dev',
  integrations: [
    starlight({
      title: 'openapi-ng',
      favicon: './favicon.svg',
      description:
        'Generate TypeScript models and Angular services from OpenAPI 3.x specs.',
      social: [
        {
          icon: 'github',
          label: 'GitHub',
          href: 'https://github.com/AVSystem/openapi-ng',
        },
        {
          icon: 'npm',
          label: 'NPM',
          href: 'https://www.npmjs.com/package/@avsystem/openapi-ng',
        },
      ],
      editLink: {
        baseUrl: 'https://github.com/AVSystem/openapi-ng/edit/main/website/',
      },
      sidebar: [
        {
          label: 'Start here',
          items: [
            { label: 'Introduction', slug: '' },
            { label: 'Getting started', slug: 'getting-started' },
          ],
        },
        {
          label: 'Guides',
          items: [
            { label: 'CLI', slug: 'guides/cli' },
            { label: 'Configuration', slug: 'guides/configuration' },
            { label: 'Angular generator', slug: 'guides/angular' },
          ],
        },
        {
          label: 'Reference',
          items: [
            { label: 'Node API', slug: 'reference/node-api' },
            { label: 'Diagnostics', slug: 'reference/diagnostics' },
            { label: 'Assumptions & limitations', slug: 'reference/limitations' },
            { label: 'Environment variables', slug: 'reference/environment' },
            { label: 'Runtime & platforms', slug: 'reference/runtime' },
          ],
        },
      ],
    }),
  ],
});
