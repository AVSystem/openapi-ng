import { EditorState, Compartment, type Extension } from '@codemirror/state';
import { EditorView, basicSetup } from 'codemirror';
import { json } from '@codemirror/lang-json';
import { yaml } from '@codemirror/lang-yaml';
import { detectFormat, type SpecFormat } from './format';
import { chrome } from './theme';

export interface SpecEditor {
  getValue(): string;
  setValue(value: string): void;
}

// Without an explicit language the editor follows the document between YAML and JSON.
export function createEditor(
  parent: HTMLElement,
  initial: string,
  onChange: (value: string) => void,
  fixedLanguage?: Extension,
): SpecEditor {
  const language = new Compartment();
  let format: SpecFormat = detectFormat(initial);

  const view = new EditorView({
    parent,
    state: EditorState.create({
      doc: initial,
      extensions: [
        basicSetup,
        chrome,
        fixedLanguage ?? language.of(format === 'json' ? json() : yaml()),
        // CodeMirror's own handler would splice the file in at the cursor.
        EditorView.domEventHandlers({
          drop(event, view) {
            const file = event.dataTransfer?.files[0];
            if (!file) return false;
            event.preventDefault();
            void file.text().then(text => {
              view.dispatch({
                changes: { from: 0, to: view.state.doc.length, insert: text },
              });
            });
            return true;
          },
        }),
        EditorView.updateListener.of(update => {
          if (!update.docChanged) return;
          const value = update.state.doc.toString();
          const next = detectFormat(value);
          if (!fixedLanguage && next !== format) {
            format = next;
            view.dispatch({
              effects: language.reconfigure(next === 'json' ? json() : yaml()),
            });
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
