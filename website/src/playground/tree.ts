export interface Artifact {
  path: string;
  contents: string;
}

export type TreeRow =
  | { kind: 'dir'; name: string; depth: 0 }
  | { kind: 'file'; path: string; name: string; depth: 0 | 1; bytes: number };

const encoder = new TextEncoder();

export function buildTree(artifacts: ReadonlyArray<Artifact>): TreeRow[] {
  const root: TreeRow[] = [];
  const dirs = new Map<string, TreeRow[]>();
  for (const artifact of artifacts) {
    const slash = artifact.path.indexOf('/');
    const bytes = encoder.encode(artifact.contents).length;
    if (slash === -1) {
      root.push({
        kind: 'file',
        path: artifact.path,
        name: artifact.path,
        depth: 0,
        bytes,
      });
      continue;
    }
    const dir = artifact.path.slice(0, slash);
    const name = artifact.path.slice(slash + 1);
    const rows = dirs.get(dir) ?? [];
    rows.push({ kind: 'file', path: artifact.path, name, depth: 1, bytes });
    dirs.set(dir, rows);
  }
  const byName = (a: { name: string }, b: { name: string }) =>
    a.name.localeCompare(b.name);
  root.sort(byName);
  for (const dir of [...dirs.keys()].sort()) {
    root.push({ kind: 'dir', name: dir, depth: 0 });
    root.push(...(dirs.get(dir) ?? []).sort(byName));
  }
  return root;
}

function formatBytes(bytes: number): string {
  return bytes < 1024 ? `${bytes} B` : `${(bytes / 1024).toFixed(1)} kB`;
}

export function renderTree(
  container: HTMLElement,
  rows: TreeRow[],
  selectedPath: string | null,
  onSelect: (path: string) => void,
): void {
  const doc = container.ownerDocument;
  const list = doc.createElement('ul');
  for (const row of rows) {
    const item = doc.createElement('li');
    item.dataset.depth = String(row.depth);
    if (row.kind === 'dir') {
      item.className = 'is-dir';
      item.textContent = `${row.name}/`;
    } else {
      item.dataset.path = row.path;
      if (row.path === selectedPath) item.className = 'is-selected';
      const button = doc.createElement('button');
      button.type = 'button';
      button.title = row.path;
      const name = doc.createElement('span');
      name.className = 'name';
      name.textContent = row.name;
      const size = doc.createElement('span');
      size.className = 'size';
      size.textContent = formatBytes(row.bytes);
      button.append(name, size);
      button.addEventListener('click', () => onSelect(row.path));
      item.append(button);
    }
    list.append(item);
  }
  container.replaceChildren(list);
}
