import hljs from 'highlight.js/lib/core';
import typescript from 'highlight.js/lib/languages/typescript';

hljs.registerLanguage('typescript', typescript);

export function renderOutput(code: HTMLElement, contents: string): void {
  code.textContent = contents;
  code.className = 'language-typescript';
  delete code.dataset.highlighted;
  hljs.highlightElement(code);
}

export async function copyText(text: string): Promise<void> {
  await navigator.clipboard.writeText(text);
}
