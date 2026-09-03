export type SpecFormat = 'json' | 'yaml';

export function detectFormat(source: string): SpecFormat {
  return source.trimStart().startsWith('{') ? 'json' : 'yaml';
}

export function displayPathFor(format: SpecFormat): 'spec.json' | 'spec.yaml' {
  return format === 'json' ? 'spec.json' : 'spec.yaml';
}
