import petstoreSpec from '../../../stackblitz/petstore.openapi.yaml?raw';
import { javascript } from '@codemirror/lang-javascript';
import type { GenerateResult, GeneratorDiagnostic } from '@avsystem/openapi-ng/browser';
import { DEFAULT_CONFIG, parseConfig } from './config';
import { createEditor } from './editor';
import { detectFormat, displayPathFor } from './format';
import { engineVersion, GenerateError, loadGenerate, type GenerateFn } from './generator';
import { copyText, createOutput } from './output';
import { buildTree, renderTree } from './tree';

const DEBOUNCE_MS = 150;
const STATUS_RESTORE_MS = 2000;
const WARMUP_SPEC = 'openapi: 3.0.3\ninfo: {title: warmup, version: "1"}\npaths: {}';

interface ConsoleLines {
  errors?: string[];
  warnings?: GeneratorDiagnostic[];
  notes?: string[];
}

function byId<T extends HTMLElement>(doc: Document, id: string): T {
  const el = doc.getElementById(id);
  if (!el) throw new Error(`playground: missing #${id}`);
  return el as T;
}

function describe(item: { code: string; subcode?: string | null; message: string }): string {
  return `${item.code}${item.subcode ? ` (${item.subcode})` : ''}: ${item.message}`;
}

export function start(doc: Document): void {
  const root = doc.querySelector<HTMLElement>('.pg');
  if (!root) return;

  if (!globalThis.crossOriginIsolated) {
    root.innerHTML =
      '<p class="pg-unsupported">The playground needs a cross-origin isolated page ' +
      '(Cross-Origin-Opener-Policy and Cross-Origin-Embedder-Policy headers). ' +
      'This host is not sending them, so the WebAssembly generator cannot start.</p>';
    return;
  }

  const status = byId<HTMLElement>(doc, 'pg-status');
  const treeEl = byId<HTMLElement>(doc, 'pg-tree');
  const summaryEl = byId<HTMLElement>(doc, 'pg-summary');
  const diagnosticsEl = byId<HTMLUListElement>(doc, 'pg-diagnostics');

  let generate: GenerateFn | null = null;
  let lastResult: GenerateResult | null = null;
  let selectedPath: string | null = null;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let runId = 0;
  let readyStatus = '';

  function announce(message: string): void {
    status.textContent = message;
    setTimeout(() => {
      status.textContent = readyStatus;
    }, STATUS_RESTORE_MS);
  }

  const editor = createEditor(byId(doc, 'pg-editor'), petstoreSpec, schedule);
  const output = createOutput(byId(doc, 'pg-code'));
  // The JSON grammar has no comments; the JavaScript one highlights JSONC fine.
  const configEditor = createEditor(byId(doc, 'pg-config'), DEFAULT_CONFIG, schedule, javascript());

  function schedule(): void {
    clearTimeout(timer);
    timer = setTimeout(run, DEBOUNCE_MS);
  }

  function renderConsole({ errors = [], warnings = [], notes = [] }: ConsoleLines): void {
    diagnosticsEl.replaceChildren();
    const append = (text: string, className?: string) => {
      const li = doc.createElement('li');
      if (className) li.className = className;
      li.textContent = text;
      diagnosticsEl.append(li);
    };
    for (const error of errors) append(error, 'is-error');
    // A diagnostic list can carry either severity; colour by what it says.
    for (const item of warnings) {
      append(describe(item), item.severity === 'error' ? 'is-error' : 'is-warning');
    }
    for (const note of notes) append(note, 'is-note');
  }

  function select(path: string): void {
    selectedPath = path;
    if (!lastResult) return;
    const rows = buildTree(lastResult.artifacts);
    renderTree(treeEl, rows, selectedPath, select);
    const artifact = lastResult.artifacts.find(a => a.path === path);
    if (artifact) output.setValue(artifact.contents);
  }

  function showResult(result: GenerateResult, elapsedMs: number, notes: string[]): void {
    lastResult = result;
    root!.classList.remove('is-stale');
    const paths = result.artifacts.map(a => a.path);
    if (!selectedPath || !paths.includes(selectedPath)) selectedPath = paths[0] ?? null;
    renderTree(treeEl, buildTree(result.artifacts), selectedPath, select);
    const current = result.artifacts.find(a => a.path === selectedPath);
    output.setValue(current ? current.contents : '');
    summaryEl.textContent =
      `${result.summary.operationCount} operations · ${result.summary.schemaCount} schemas · ` +
      `${result.artifacts.length} files · ${elapsedMs.toFixed(0)} ms`;
    renderConsole({ warnings: result.diagnostics, notes });
  }

  async function run(): Promise<void> {
    if (!generate) return;
    // Guards against out-of-order resolution: a later edit's generation can
    // resolve before an earlier, slower one still in flight.
    const id = ++runId;
    const source = editor.getValue();
    const config = parseConfig(configEditor.getValue());
    if (!config.ok) {
      root!.classList.add('is-stale');
      summaryEl.textContent = 'config error';
      renderConsole({ errors: [config.error] });
      return;
    }
    const started = performance.now();
    try {
      // No inputFormat: the /browser subpath's InputFormat is a const enum with no
      // runtime export, and displayPath's extension already selects the decoder.
      const result = await generate({
        inputContents: source,
        displayPath: displayPathFor(detectFormat(source)),
        ...config.options,
      });
      if (id !== runId) return;
      showResult(result, performance.now() - started, config.notes);
    } catch (err) {
      if (id !== runId) return;
      root!.classList.add('is-stale');
      if (err instanceof GenerateError) {
        summaryEl.textContent = 'generation failed';
        renderConsole({ errors: [describe(err)], warnings: err.warnings, notes: config.notes });
      } else {
        summaryEl.textContent = err instanceof Error ? err.message : String(err);
        renderConsole({ notes: config.notes });
      }
    } finally {
      // Lets tests and tooling wait for a specific edit's generation to settle.
      if (id === runId) root!.dataset.run = String(id);
    }
  }

  const fileInput = byId<HTMLInputElement>(doc, 'pg-file');
  byId<HTMLButtonElement>(doc, 'pg-open').addEventListener('click', () => fileInput.click());
  fileInput.addEventListener('change', async () => {
    const file = fileInput.files?.[0];
    // Cleared so picking the same file again still fires `change`.
    fileInput.value = '';
    if (file) editor.setValue(await file.text());
  });
  byId<HTMLButtonElement>(doc, 'pg-reset').addEventListener('click', () => {
    editor.setValue(petstoreSpec);
    configEditor.setValue(DEFAULT_CONFIG);
  });
  byId<HTMLButtonElement>(doc, 'pg-copy').addEventListener('click', async () => {
    const artifact = lastResult?.artifacts.find(a => a.path === selectedPath);
    if (!artifact) return;
    try {
      await copyText(artifact.contents);
    } catch (err) {
      announce(`Copy failed: ${err instanceof Error ? err.message : String(err)}`);
    }
  });

  const fn = loadGenerate();
  fn({ inputContents: WARMUP_SPEC, displayPath: 'spec.yaml', emit: ['models'] })
    .then(() => {
      generate = fn;
      return engineVersion()
        .then(version => `openapi-ng v${version} · runs in your browser`)
        .catch(() => 'openapi-ng · runs in your browser');
    })
    .then(text => {
      readyStatus = text;
      status.textContent = text;
      return run();
    })
    .catch((err: unknown) => {
      status.textContent = err instanceof Error ? err.message : String(err);
    });
}
