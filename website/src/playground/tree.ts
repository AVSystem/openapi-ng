export interface Artifact {
  path: string;
  contents: string;
}

export type TreeNode =
  | { kind: 'dir'; path: string; name: string; children: TreeNode[] }
  | { kind: 'file'; path: string; name: string; bytes: number };

export interface TreeView {
  selectedPath: string | null;
  // Directory paths folded shut; everything else is open.
  collapsed: ReadonlySet<string>;
  onSelect(path: string): void;
  onToggle(dir: string): void;
}

const encoder = new TextEncoder();

export function buildTree(artifacts: ReadonlyArray<Artifact>): TreeNode[] {
  const root: TreeNode[] = [];
  const dirs = new Map<string, TreeNode[]>();
  const childrenOf = (dir: string): TreeNode[] => {
    if (dir === '') return root;
    let children = dirs.get(dir);
    if (!children) {
      children = [];
      dirs.set(dir, children);
      const slash = dir.lastIndexOf('/');
      childrenOf(slash === -1 ? '' : dir.slice(0, slash)).push({
        kind: 'dir',
        path: dir,
        name: dir.slice(slash + 1),
        children,
      });
    }
    return children;
  };
  for (const artifact of artifacts) {
    const slash = artifact.path.lastIndexOf('/');
    childrenOf(slash === -1 ? '' : artifact.path.slice(0, slash)).push({
      kind: 'file',
      path: artifact.path,
      name: artifact.path.slice(slash + 1),
      bytes: encoder.encode(artifact.contents).length,
    });
  }
  const sort = (nodes: TreeNode[]) => {
    nodes.sort(
      (a, b) =>
        Number(a.kind === 'dir') - Number(b.kind === 'dir') ||
        a.name.localeCompare(b.name),
    );
    for (const node of nodes) if (node.kind === 'dir') sort(node.children);
  };
  sort(root);
  return root;
}

// Directories on the way to `path`, outermost first.
export function ancestorsOf(path: string): string[] {
  const dirs: string[] = [];
  for (let i = path.indexOf('/'); i !== -1; i = path.indexOf('/', i + 1)) {
    dirs.push(path.slice(0, i));
  }
  return dirs;
}

function formatBytes(bytes: number): string {
  return bytes < 1024 ? `${bytes} B` : `${(bytes / 1024).toFixed(1)} kB`;
}

export function renderTree(
  container: HTMLElement,
  nodes: TreeNode[],
  view: TreeView,
): void {
  const doc = container.ownerDocument;
  const render = (children: TreeNode[]): HTMLUListElement => {
    const list = doc.createElement('ul');
    for (const node of children) {
      const item = doc.createElement('li');
      const button = doc.createElement('button');
      button.type = 'button';
      button.title = node.path;
      const name = doc.createElement('span');
      name.className = 'name';
      name.textContent = node.name;
      button.append(name);
      if (node.kind === 'dir') {
        const open = !view.collapsed.has(node.path);
        item.className = open ? 'is-dir' : 'is-dir is-collapsed';
        item.dataset.dir = node.path;
        button.setAttribute('aria-expanded', String(open));
        button.addEventListener('click', () => view.onToggle(node.path));
        item.append(button, render(node.children));
      } else {
        item.dataset.path = node.path;
        if (node.path === view.selectedPath) item.className = 'is-selected';
        const size = doc.createElement('span');
        size.className = 'size';
        size.textContent = formatBytes(node.bytes);
        button.append(size);
        button.addEventListener('click', () => view.onSelect(node.path));
        item.append(button);
      }
      list.append(item);
    }
    return list;
  };
  container.replaceChildren(render(nodes));
}
