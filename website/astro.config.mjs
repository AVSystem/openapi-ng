import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

// `astro dev` ignores public/_headers; mirror its COOP/COEP scope so the
// playground page is cross-origin isolated and its wasm worker inherits COEP.
const playgroundHeaders = {
  name: 'playground-headers',
  configureServer(server) {
    server.middlewares.use((req, res, next) => {
      const pathname = req.url?.split('?')[0] ?? '';
      if (
        pathname === '/playground' ||
        pathname.startsWith('/playground/') ||
        pathname.startsWith('/playground-engine/')
      ) {
        res.setHeader('Cross-Origin-Opener-Policy', 'same-origin');
        res.setHeader('Cross-Origin-Embedder-Policy', 'require-corp');
      }
      next();
    });
  },
};

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
            { label: 'Playground', link: '/playground/' },
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
  vite: {
    build: { target: 'es2022' },
    server: { fs: { allow: ['..'] } },
    plugins: [playgroundHeaders],
    // The playground's imports are only reachable through src/playground/*.ts, so
    // dev discovers them late and re-optimizes, which 504s already-served modules.
    optimizeDeps: {
      include: [
        '@avsystem/openapi-ng/browser',
        'codemirror',
        '@codemirror/state',
        '@codemirror/lang-json',
        '@codemirror/lang-yaml',
        'highlight.js/lib/core',
        'highlight.js/lib/languages/typescript',
        'lz-string',
      ],
    },
  },
});
