import { javascript } from '@codemirror/lang-javascript';
import { EditorState } from '@codemirror/state';
import { EditorView, basicSetup } from 'codemirror';
import { chrome } from './theme';

export interface OutputView {
  setValue(contents: string): void;
}

export function createOutput(parent: HTMLElement): OutputView {
  const view = new EditorView({
    parent,
    state: EditorState.create({
      extensions: [
        basicSetup,
        chrome,
        javascript({ typescript: true }),
        EditorState.readOnly.of(true),
        EditorView.editable.of(false),
        // Non-editable content is not focusable by default, which would leave
        // the search keymap unreachable.
        EditorView.contentAttributes.of({ tabindex: '0' }),
      ],
    }),
  });
  return {
    setValue: contents =>
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: contents },
      }),
  };
}

export async function copyText(text: string): Promise<void> {
  await navigator.clipboard.writeText(text);
}
