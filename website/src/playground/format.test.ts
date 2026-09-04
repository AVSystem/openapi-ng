import { describe, expect, it } from 'vitest';
import { detectFormat, displayPathFor } from './format';

describe('detectFormat', () => {
  it('treats a leading brace as JSON', () => {
    expect(detectFormat('  \n{ "openapi": "3.0.3" }')).toBe('json');
  });
  it('treats anything else as YAML', () => {
    expect(detectFormat('openapi: 3.0.3')).toBe('yaml');
    expect(detectFormat('')).toBe('yaml');
  });
});

describe('displayPathFor', () => {
  it('maps format to a file name with the matching extension', () => {
    expect(displayPathFor('json')).toBe('spec.json');
    expect(displayPathFor('yaml')).toBe('spec.yaml');
  });
});
