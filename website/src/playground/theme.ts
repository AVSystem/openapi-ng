import { HighlightStyle, syntaxHighlighting } from '@codemirror/language';
import type { Extension } from '@codemirror/state';
import { tags } from '@lezer/highlight';
import { EditorView } from 'codemirror';

// Every editor root carries the `hljs` class, so the highlight.js theme
// supplies the colors and background for all panes.
const theme = EditorView.theme({
  '&': { height: '100%' },
  '.cm-gutters': { backgroundColor: 'var(--sl-color-bg-nav)', color: 'var(--sl-color-gray-3)', border: 'none' },
  '.cm-activeLine, .cm-activeLineGutter': { backgroundColor: 'var(--sl-color-gray-6)' },
  '.cm-content': { fontFamily: 'var(--__sl-font-mono)' },
  '.cm-cursor': { borderLeftColor: 'currentColor' },
  // Mirrors the base theme's own selector depth, which a shorter one loses to.
  // Its default tint is invisible against the dark hljs background.
  '.cm-selectionBackground, &.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, ::selection':
    { backgroundColor: 'var(--sl-color-accent-low)' },
  '.cm-panels': { backgroundColor: 'var(--sl-color-bg-nav)', color: 'var(--sl-color-white)' },
  '.cm-panels.cm-panels-top': { borderBottom: '1px solid var(--sl-color-gray-5)' },
  '.cm-panel input, .cm-panel button': {
    background: 'var(--sl-color-bg)',
    color: 'var(--sl-color-white)',
    border: '1px solid var(--sl-color-gray-5)',
    borderRadius: '0.25rem',
  },
  '.cm-searchMatch': { backgroundColor: 'var(--sl-color-orange-low)' },
  '.cm-searchMatch.cm-searchMatch-selected': { backgroundColor: 'var(--sl-color-orange)' },
});

// Token classes mirror what highlight.js emits for YAML, JSON and TypeScript.
const highlight = HighlightStyle.define([
  { tag: tags.keyword, class: 'hljs-keyword' },
  { tag: [tags.typeName, tags.className], class: 'hljs-title class_' },
  { tag: [tags.function(tags.variableName), tags.function(tags.propertyName)], class: 'hljs-title function_' },
  { tag: [tags.bool, tags.null], class: 'hljs-literal' },
  { tag: tags.number, class: 'hljs-number' },
  { tag: [tags.propertyName, tags.definition(tags.propertyName)], class: 'hljs-attr' },
  { tag: [tags.string, tags.special(tags.string)], class: 'hljs-string' },
  { tag: tags.labelName, class: 'hljs-symbol' },
  { tag: [tags.comment, tags.meta], class: 'hljs-comment' },
  { tag: tags.invalid, class: 'hljs-deletion' },
]);

export const chrome: Extension = [
  theme,
  syntaxHighlighting(highlight),
  EditorView.editorAttributes.of({ class: 'hljs' }),
];
