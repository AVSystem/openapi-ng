import { EditorState, Compartment } from '@codemirror/state';
import { EditorView, basicSetup } from 'codemirror';
import { json } from '@codemirror/lang-json';
import { yaml } from '@codemirror/lang-yaml';
import { detectFormat, type SpecFormat } from './format';

export interface SpecEditor {
  getValue(): string;
  setValue(value: string): void;
}

const theme = EditorView.theme({
  '&': { height: '100%', backgroundColor: 'var(--sl-color-bg)', color: 'var(--sl-color-text)' },
  '.cm-gutters': { backgroundColor: 'var(--sl-color-bg-nav)', color: 'var(--sl-color-gray-3)', border: 'none' },
  '.cm-activeLine, .cm-activeLineGutter': { backgroundColor: 'var(--sl-color-gray-6)' },
  '.cm-content': { fontFamily: 'var(--__sl-font-mono)' },
});

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
