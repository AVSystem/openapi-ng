import { yamlLanguage } from '@codemirror/lang-yaml';
import { HighlightStyle, syntaxHighlighting } from '@codemirror/language';
import type { Extension } from '@codemirror/state';
import { tags } from '@lezer/highlight';
import { EditorView } from 'codemirror';

// Colours come from the --pg-* palette in playground.css, which follows
// Starlight's theme attribute.
const theme = EditorView.theme({
  '&': { height: '100%', backgroundColor: 'var(--pg-bg)', color: 'var(--pg-fg)' },
  '.cm-gutters': { backgroundColor: 'var(--pg-bg)', color: 'var(--pg-gutter)', border: 'none' },
  '.cm-activeLineGutter': { backgroundColor: 'var(--pg-active-line)', color: 'var(--pg-gutter-active)' },
  '.cm-activeLine': { backgroundColor: 'var(--pg-active-line)' },
  '.cm-content': { fontFamily: 'var(--__sl-font-mono)', caretColor: 'var(--pg-cursor)' },
  '.cm-cursor': { borderLeftColor: 'var(--pg-cursor)' },
  // Mirrors the base theme's own selector depth, which a shorter one loses to.
  '.cm-selectionBackground, &.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, ::selection':
    { backgroundColor: 'var(--pg-selection)' },
  '.cm-selectionMatch, &.cm-focused .cm-matchingBracket': { backgroundColor: 'var(--pg-bracket)' },
  '.cm-searchMatch': { backgroundColor: 'var(--pg-match)' },
  '.cm-searchMatch.cm-searchMatch-selected': { backgroundColor: 'var(--pg-match-selected)' },
  '.cm-panels': { backgroundColor: 'var(--sl-color-bg-nav)', color: 'var(--sl-color-white)' },
  '.cm-panels.cm-panels-top': { borderBottom: '1px solid var(--sl-color-gray-5)' },
  '.cm-panel input, .cm-panel button': {
    background: 'var(--sl-color-bg)',
    color: 'var(--sl-color-white)',
    border: '1px solid var(--sl-color-gray-5)',
    borderRadius: '0.25rem',
  },
});

// Follows how the docs render TypeScript: object keys stay plain, member
// accesses are tinted, the decorator sigil takes the function colour.
const highlight = HighlightStyle.define([
  { tag: tags.keyword, class: 'tok-keyword' },
  { tag: [tags.typeName, tags.className, tags.namespace], class: 'tok-type' },
  { tag: [tags.function(tags.variableName), tags.function(tags.propertyName), tags.meta], class: 'tok-function' },
  { tag: tags.definition(tags.propertyName), class: 'tok-plain' },
  { tag: [tags.propertyName, tags.labelName], class: 'tok-property' },
  { tag: [tags.string, tags.special(tags.string)], class: 'tok-string' },
  { tag: tags.special(tags.brace), class: 'tok-interpolation' },
  { tag: tags.regexp, class: 'tok-regexp' },
  { tag: [tags.escape, tags.number], class: 'tok-number' },
  { tag: [tags.bool, tags.null, tags.atom], class: 'tok-literal' },
  { tag: [tags.operator, tags.self], class: 'tok-operator' },
  { tag: tags.comment, class: 'tok-comment' },
  { tag: tags.invalid, class: 'tok-invalid' },
]);

// YAML keys and plain scalars have their own colours in the docs; the
// base style would leave both in the text colour.
const yamlHighlight = HighlightStyle.define(
  [
    { tag: tags.definition(tags.propertyName), class: 'tok-yaml-key' },
    { tag: [tags.content, tags.string, tags.special(tags.string)], class: 'tok-yaml-value' },
  ],
  { scope: yamlLanguage },
);

export const chrome: Extension = [theme, syntaxHighlighting(highlight), syntaxHighlighting(yamlHighlight)];
