import type { GenerateOptions } from '@avsystem/openapi-ng/browser';

export type ConfigOptions = Pick<
  GenerateOptions,
  'emit' | 'mappedTypes' | 'responseTypeMapping' | 'naming'
>;

export type ParsedConfig =
  | { ok: true; options: ConfigOptions; notes: string[] }
  | { ok: false; error: string };

export const DEFAULT_CONFIG = '{\n  "emit": ["models", "angular"]\n}\n';

const REGEX_LITERAL = /^\/(.*)\/([a-z]*)$/s;

class ConfigError extends Error {}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

// JSON cannot carry a RegExp; accept the literal spelling from a TS config
// as a string and lower it to the {source, flags} shape the wrapper takes.
function lowerRule(rule: unknown, path: string): unknown {
  if (!isRecord(rule) || typeof rule.parse !== 'string') return rule;
  const match = REGEX_LITERAL.exec(rule.parse);
  if (!match) {
    throw new ConfigError(`${path}.parse: write the regex as a literal, e.g. "/^abc$/i"`);
  }
  return { ...rule, parse: { source: match[1], flags: match[2] } };
}

function lowerNaming(value: unknown, path: string): unknown {
  if (Array.isArray(value)) return value.map((item, i) => lowerRule(item, `${path}[${i}]`));
  return lowerRule(value, path);
}

export function parseConfig(text: string): ParsedConfig {
  let doc: unknown;
  try {
    doc = JSON.parse(text);
  } catch (err) {
    return { ok: false, error: `config is not valid JSON: ${(err as Error).message}` };
  }
  if (!isRecord(doc)) return { ok: false, error: 'config must be a JSON object' };

  const { input, output, ...options } = doc;
  const notes: string[] = [];
  if (input !== undefined || output !== undefined) {
    notes.push('input and output are ignored in the playground');
  }
  try {
    if (isRecord(options.naming)) {
      options.naming = Object.fromEntries(
        Object.entries(options.naming).map(([key, value]) => [
          key,
          lowerNaming(value, `naming.${key}`),
        ]),
      );
    }
  } catch (err) {
    if (err instanceof ConfigError) return { ok: false, error: err.message };
    throw err;
  }
  return { ok: true, options: options as ConfigOptions, notes };
}
