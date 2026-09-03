import petstoreSpec from '../../../stackblitz/petstore.openapi.yaml?raw';
import type { GenerateResult, GeneratorDiagnostic } from '@avsystem/openapi-ng/browser';
import { createEditor } from './editor';
import { detectFormat, displayPathFor } from './format';
import { engineVersion, GenerateError, loadGenerate, type GenerateFn } from './generator';
import { copyText, renderOutput } from './output';
import { decodeShareHash, encodeShareHash } from './share';
import { buildTree, renderTree } from './tree';

const DEBOUNCE_MS = 150;
const STATUS_RESTORE_MS = 2000;
const WARMUP_SPEC = 'openapi: 3.0.3\ninfo: {title: warmup, version: "1"}\npaths: {}';

function byId<T extends HTMLElement>(doc: Document, id: string): T {
  const el = doc.getElementById(id);
  if (!el) throw new Error(`playground: missing #${id}`);
  return el as T;
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
  const codeEl = byId<HTMLElement>(doc, 'pg-code');
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

  const initial = decodeShareHash(location.hash) ?? petstoreSpec;
  const editor = createEditor(byId(doc, 'pg-editor'), initial, schedule);

  function schedule(): void {
    clearTimeout(timer);
    timer = setTimeout(run, DEBOUNCE_MS);
  }

  function renderDiagnostics(items: GeneratorDiagnostic[], fatal?: GenerateError): void {
    diagnosticsEl.replaceChildren();
    if (fatal) {
      const li = doc.createElement('li');
      li.className = 'is-error';
      li.textContent = `${fatal.code}${fatal.subcode ? ` (${fatal.subcode})` : ''}: ${fatal.message}`;
      diagnosticsEl.append(li);
    }
    for (const item of items) {
      const li = doc.createElement('li');
      li.textContent = `${item.code}${item.subcode ? ` (${item.subcode})` : ''}: ${item.message}`;
      diagnosticsEl.append(li);
    }
  }

  function select(path: string): void {
    selectedPath = path;
    if (!lastResult) return;
    const rows = buildTree(lastResult.artifacts);
    renderTree(treeEl, rows, selectedPath, select);
    const artifact = lastResult.artifacts.find(a => a.path === path);
    if (artifact) renderOutput(codeEl, artifact.contents);
  }

  function showResult(result: GenerateResult, elapsedMs: number): void {
    lastResult = result;
    root!.classList.remove('is-stale');
    const paths = result.artifacts.map(a => a.path);
    if (!selectedPath || !paths.includes(selectedPath)) selectedPath = paths[0] ?? null;
    renderTree(treeEl, buildTree(result.artifacts), selectedPath, select);
    const current = result.artifacts.find(a => a.path === selectedPath);
    renderOutput(codeEl, current ? current.contents : '');
    summaryEl.textContent =
      `${result.summary.operationCount} operations · ${result.summary.schemaCount} schemas · ` +
      `${result.artifacts.length} files · ${elapsedMs.toFixed(0)} ms`;
    renderDiagnostics(result.diagnostics);
  }

  async function run(): Promise<void> {
    if (!generate) return;
    // Guards against out-of-order resolution: a later edit's generation can
    // resolve before an earlier, slower one still in flight.
    const id = ++runId;
    const source = editor.getValue();
    const started = performance.now();
    try {
      // No inputFormat: the /browser subpath's InputFormat is a const enum with no
      // runtime export, and displayPath's extension already selects the decoder.
      const result = await generate({
        inputContents: source,
        displayPath: displayPathFor(detectFormat(source)),
        emit: ['models', 'angular'],
      });
      if (id !== runId) return;
      showResult(result, performance.now() - started);
    } catch (err) {
      if (id !== runId) return;
      root!.classList.add('is-stale');
      if (err instanceof GenerateError) {
        summaryEl.textContent = 'generation failed';
        renderDiagnostics(err.warnings, err);
      } else {
        renderDiagnostics([]);
        summaryEl.textContent = err instanceof Error ? err.message : String(err);
      }
    }
  }

  byId<HTMLButtonElement>(doc, 'pg-reset').addEventListener('click', () => {
    editor.setValue(petstoreSpec);
    history.replaceState(null, '', location.pathname);
  });
  byId<HTMLButtonElement>(doc, 'pg-share').addEventListener('click', async () => {
    const hash = encodeShareHash(editor.getValue());
    history.replaceState(null, '', location.pathname + hash);
    try {
      await copyText(location.href);
      announce('Link copied');
    } catch (err) {
      announce(`Copy failed: ${err instanceof Error ? err.message : String(err)}`);
    }
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
