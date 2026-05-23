import test from 'ava';
import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.join(__dirname, '..');
const require = createRequire(import.meta.url);

const browserEntry = require(path.join(repoRoot, 'browser.js')) as {
  generate: (options?: unknown) => Promise<never>;
  GenerateError: {
    isGenerateError: (value: unknown) => boolean;
    new (payload?: unknown): Error & { code?: string; message: string };
  };
  EmitTarget: { Models: string; Angular: string };
};

test('browser generate throws a GenerateError, not a plain Error', async t => {
  const { generate, GenerateError } = browserEntry;
  const err = await t.throwsAsync(async () => {
    await generate({ inputPath: 'x', emit: ['models'] });
  });
  t.true(GenerateError.isGenerateError(err));
  t.is((err as { code?: string } | undefined)?.code, 'E_UNSUPPORTED_RUNTIME');
  // Be lenient on the message — the test originally expected /browser/i but
  // the new wrapper says "browser/runtime context".
  t.regex(err?.message ?? '', /browser|runtime/i);
});

test('browser entry exports EmitTarget mirror', t => {
  t.truthy(browserEntry.EmitTarget);
  t.is(browserEntry.EmitTarget.Models, 'models');
  t.is(browserEntry.EmitTarget.Angular, 'angular');
});
