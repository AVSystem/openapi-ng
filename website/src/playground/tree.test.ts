import { describe, expect, it } from 'vitest';
import { ancestorsOf, buildTree } from './tree';

const artifact = (path: string, contents = 'x') => ({ path, contents });

describe('buildTree', () => {
  it('lists files alphabetically, then directories, at every depth', () => {
    const nodes = buildTree([
      artifact('rest/pet/list-pets.ts', 'abc'),
      artifact('rest.util.ts'),
      artifact('model.ts'),
      artifact('rest/pet.rest.ts'),
      artifact('rest/pet/index.ts'),
      artifact('rest/account/get-account.ts'),
    ]);
    expect(nodes).toEqual([
      { kind: 'file', path: 'model.ts', name: 'model.ts', bytes: 1 },
      { kind: 'file', path: 'rest.util.ts', name: 'rest.util.ts', bytes: 1 },
      {
        kind: 'dir',
        path: 'rest',
        name: 'rest',
        children: [
          {
            kind: 'file',
            path: 'rest/pet.rest.ts',
            name: 'pet.rest.ts',
            bytes: 1,
          },
          {
            kind: 'dir',
            path: 'rest/account',
            name: 'account',
            children: [
              {
                kind: 'file',
                path: 'rest/account/get-account.ts',
                name: 'get-account.ts',
                bytes: 1,
              },
            ],
          },
          {
            kind: 'dir',
            path: 'rest/pet',
            name: 'pet',
            children: [
              {
                kind: 'file',
                path: 'rest/pet/index.ts',
                name: 'index.ts',
                bytes: 1,
              },
              {
                kind: 'file',
                path: 'rest/pet/list-pets.ts',
                name: 'list-pets.ts',
                bytes: 3,
              },
            ],
          },
        ],
      },
    ]);
  });
  it('measures bytes as UTF-8', () => {
    expect(buildTree([artifact('a.ts', 'é')])[0]).toMatchObject({ bytes: 2 });
  });
});

describe('ancestorsOf', () => {
  it('walks from the outermost directory down', () => {
    expect(ancestorsOf('rest/pet/index.ts')).toEqual(['rest', 'rest/pet']);
    expect(ancestorsOf('model.ts')).toEqual([]);
  });
});
