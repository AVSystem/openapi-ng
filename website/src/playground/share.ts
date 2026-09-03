import LZString from 'lz-string';

const PREFIX = '#spec=';

export function encodeShareHash(spec: string): string {
  return PREFIX + LZString.compressToEncodedURIComponent(spec);
}

// lz-string returns '' or null for input it cannot decode; both mean "no spec".
export function decodeShareHash(hash: string): string | null {
  if (!hash.startsWith(PREFIX)) return null;
  const payload = hash.slice(PREFIX.length);
  if (payload === '') return null;
  const decoded = LZString.decompressFromEncodedURIComponent(payload);
  return decoded ? decoded : null;
}
