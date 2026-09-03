import { describe, expect, it } from 'vitest';
import { buildTree } from './tree';

const artifact = (path: string, contents = 'x') => ({ path, contents });

describe('buildTree', () => {
  it('lists root files alphabetically, then each directory with its files', () => {
    const rows = buildTree([
      artifact('rest/pet.rest.generated.ts', 'abc'),
      artifact('rest.util.ts'),
      artifact('model.generated.ts'),
      artifact('rest/account.rest.generated.ts'),
    ]);
    expect(rows).toEqual([
      { kind: 'file', path: 'model.generated.ts', name: 'model.generated.ts', depth: 0, bytes: 1 },
      { kind: 'file', path: 'rest.util.ts', name: 'rest.util.ts', depth: 0, bytes: 1 },
      { kind: 'dir', name: 'rest', depth: 0 },
      { kind: 'file', path: 'rest/account.rest.generated.ts', name: 'account.rest.generated.ts', depth: 1, bytes: 1 },
      { kind: 'file', path: 'rest/pet.rest.generated.ts', name: 'pet.rest.generated.ts', depth: 1, bytes: 3 },
    ]);
  });
  it('measures bytes as UTF-8', () => {
    expect(buildTree([artifact('a.ts', 'é')])[0]).toMatchObject({ bytes: 2 });
  });
});
