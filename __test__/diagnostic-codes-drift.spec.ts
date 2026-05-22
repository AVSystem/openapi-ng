import test from 'ava';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { resolve, dirname } from 'node:path';
import ts from 'typescript';

const here = dirname(fileURLToPath(import.meta.url));
const dtsInPath = resolve(here, '..', 'index.d.ts.in');
const diagnosticsPagePath = resolve(
  here,
  '..',
  'website',
  'src',
  'content',
  'docs',
  'reference',
  'diagnostics.md',
);

const dtsIn = readFileSync(dtsInPath, 'utf8');
const diagnosticsPage = readFileSync(diagnosticsPagePath, 'utf8');

// Pull every ```ts ... ``` fenced block out of the Starlight page and
// concatenate them: the page intentionally splits `DiagnosticCode` and
// `DiagnosticSubcode` across separate snippets. Joining lets us pass
// the result to a single TS parse.
function tsBlocksFromMarkdown(md: string): string {
  const out: string[] = [];
  const fence = /```ts\n([\s\S]*?)\n```/g;
  let m: RegExpExecArray | null;
  while ((m = fence.exec(md)) !== null) {
    out.push(m[1]);
  }
  return out.join('\n\n');
}

// Parse the source as TypeScript and pull every string-literal member
// of the union assigned to `type <name> = ...`. AST-based to ride out
// reformatting (comments, trailing commas in unions, alternative join
// styles) that a regex would silently mis-handle.
function extractUnion(source: string, name: string): Set<string> {
  const sf = ts.createSourceFile(
    `${name}.ts`,
    source,
    ts.ScriptTarget.Latest,
    /* setParentNodes */ true,
    ts.ScriptKind.TS,
  );
  let found: ts.TypeAliasDeclaration | null = null;
  sf.forEachChild(node => {
    if (ts.isTypeAliasDeclaration(node) && node.name.text === name) {
      found = node;
    }
  });
  if (!found) throw new Error(`${name} type alias not found`);
  const aliased = (found as ts.TypeAliasDeclaration).type;
  const members: ts.TypeNode[] = ts.isUnionTypeNode(aliased)
    ? [...aliased.types]
    : [aliased];
  const values = new Set<string>();
  for (const member of members) {
    if (ts.isLiteralTypeNode(member) && ts.isStringLiteral(member.literal)) {
      values.add(member.literal.text);
    } else {
      throw new Error(
        `${name} union contains non-string-literal member: ${member.getText(sf)}`,
      );
    }
  }
  return values;
}

const docSource = tsBlocksFromMarkdown(diagnosticsPage);

for (const name of ['DiagnosticCode', 'DiagnosticSubcode']) {
  test(`${name}: Starlight diagnostics page matches index.d.ts.in`, t => {
    const truth = extractUnion(dtsIn, name);
    const documented = extractUnion(docSource, name);
    t.true(truth.size > 0, `${name} extraction must be non-empty`);
    t.deepEqual([...documented].sort(), [...truth].sort());
  });
}
