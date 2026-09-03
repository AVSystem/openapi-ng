import { describe, expect, it } from 'vitest';
import { DEFAULT_CONFIG, parseConfig } from './config';

describe('parseConfig', () => {
  it('passes generator options through', () => {
    const text = JSON.stringify({
      emit: ['models'],
      mappedTypes: [{ schema: 'GeoJSON', import: 'geojson', type: 'GeoJSON', alias: 'NativeGeoJSON' }],
      responseTypeMapping: [{ contentType: 'application/pdf', responseType: 'blob' }],
    });
    expect(parseConfig(text)).toEqual({
      ok: true,
      options: JSON.parse(text),
      notes: [],
    });
  });

  it('drops input and output with a note', () => {
    const result = parseConfig('{"input": "./spec.yaml", "output": "./out", "emit": ["angular"]}');
    expect(result).toEqual({
      ok: true,
      options: { emit: ['angular'] },
      notes: ['input and output are ignored in the playground'],
    });
  });

  it('lowers a regex literal string in a methodName rule to {source, flags}', () => {
    const result = parseConfig(
      JSON.stringify({
        naming: {
          methodName: {
            from: '{operationId}',
            parse: '/^(?:[^_]+_)+(?<rest>.+)$/i',
            format: '{capture.rest}',
            case: 'camel',
          },
        },
      }),
    );
    expect(result).toMatchObject({
      ok: true,
      options: {
        naming: {
          methodName: {
            from: '{operationId}',
            parse: { source: '^(?:[^_]+_)+(?<rest>.+)$', flags: 'i' },
            format: '{capture.rest}',
            case: 'camel',
          },
        },
      },
    });
  });

  it('lowers parse inside a group chain and keeps string items', () => {
    const result = parseConfig(
      JSON.stringify({
        naming: { group: ['{tags[0]}', { from: '{path}', parse: '/^\\/(?<seg>[^/]+)/', format: '{capture.seg}' }] },
      }),
    );
    expect(result).toMatchObject({
      ok: true,
      options: {
        naming: {
          group: ['{tags[0]}', { from: '{path}', parse: { source: '^\\/(?<seg>[^/]+)', flags: '' }, format: '{capture.seg}' }],
        },
      },
    });
  });

  it('keeps an explicit {source, flags} parse untouched', () => {
    const result = parseConfig(
      JSON.stringify({ naming: { methodName: { parse: { source: 'a', flags: 'g' }, format: 'x' } } }),
    );
    expect(result).toMatchObject({
      ok: true,
      options: { naming: { methodName: { parse: { source: 'a', flags: 'g' }, format: 'x' } } },
    });
  });

  it('rejects a parse string that is not a regex literal', () => {
    const result = parseConfig(JSON.stringify({ naming: { methodName: { parse: '^abc$' } } }));
    expect(result).toEqual({
      ok: false,
      error: 'naming.methodName.parse: write the regex as a literal, e.g. "/^abc$/i"',
    });
  });

  it('rejects invalid JSON', () => {
    const result = parseConfig('{"emit": [');
    expect(result.ok).toBe(false);
    if (!result.ok) expect(result.error).toMatch(/^config is not valid JSON: /);
  });

  it('rejects a non-object document', () => {
    expect(parseConfig('["models"]')).toEqual({ ok: false, error: 'config must be a JSON object' });
    expect(parseConfig('null')).toEqual({ ok: false, error: 'config must be a JSON object' });
  });

  it('ships a default that parses to the default emit set', () => {
    expect(parseConfig(DEFAULT_CONFIG)).toEqual({
      ok: true,
      options: { emit: ['models', 'angular'] },
      notes: [],
    });
  });
});
