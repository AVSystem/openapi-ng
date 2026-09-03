import { HighlightStyle, syntaxHighlighting } from '@codemirror/language';
import { EditorState, Compartment } from '@codemirror/state';
import { tags } from '@lezer/highlight';
import { EditorView, basicSetup } from 'codemirror';
import { json } from '@codemirror/lang-json';
import { yaml } from '@codemirror/lang-yaml';
import { detectFormat, type SpecFormat } from './format';

export interface SpecEditor {
  getValue(): string;
  setValue(value: string): void;
}

// The editor root carries the `hljs` class, so the highlight.js theme that
// styles the output pane also supplies the editor's colors and background.
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
});

// Token classes mirror what highlight.js emits for YAML and JSON.
const highlight = HighlightStyle.define([
  { tag: [tags.keyword, tags.typeName], class: 'hljs-keyword' },
  { tag: [tags.bool, tags.null, tags.number], class: 'hljs-literal' },
  { tag: [tags.propertyName, tags.definition(tags.propertyName)], class: 'hljs-attr' },
  { tag: [tags.string, tags.special(tags.string)], class: 'hljs-string' },
  { tag: tags.labelName, class: 'hljs-symbol' },
  { tag: [tags.comment, tags.meta], class: 'hljs-comment' },
  { tag: tags.invalid, class: 'hljs-deletion' },
]);

export function createEditor(
  parent: HTMLElement,
  initial: string,
  onChange: (value: string) => void,
): SpecEditor {
  const language = new Compartment();
  let format: SpecFormat = detectFormat(initial);

  const view = new EditorView({
    parent,
    state: EditorState.create({
      doc: initial,
      extensions: [
        basicSetup,
        theme,
        syntaxHighlighting(highlight),
        EditorView.editorAttributes.of({ class: 'hljs' }),
        language.of(format === 'json' ? json() : yaml()),
        EditorView.updateListener.of(update => {
          if (!update.docChanged) return;
          const value = update.state.doc.toString();
          const next = detectFormat(value);
          if (next !== format) {
            format = next;
            view.dispatch({ effects: language.reconfigure(next === 'json' ? json() : yaml()) });
          }
          onChange(value);
        }),
      ],
    }),
  });

  return {
    getValue: () => view.state.doc.toString(),
    setValue: value =>
      view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: value } }),
  };
}
