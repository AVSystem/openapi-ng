import { describe, expect, it } from 'vitest';
import { decodeShareHash, encodeShareHash } from './share';

describe('share hash', () => {
  it('round-trips a spec through the URL hash', () => {
    const spec = 'openapi: 3.0.3\ninfo:\n  title: Ünïcode & spaces\n';
    const hash = encodeShareHash(spec);
    expect(hash.startsWith('#spec=')).toBe(true);
    expect(decodeShareHash(hash)).toBe(spec);
  });
  it('returns null for missing, foreign or corrupt hashes', () => {
    expect(decodeShareHash('')).toBeNull();
    expect(decodeShareHash('#section-anchor')).toBeNull();
    expect(decodeShareHash('#spec=')).toBeNull();
    expect(decodeShareHash('#spec=%%%not-lz%%%')).toBeNull();
  });
});
