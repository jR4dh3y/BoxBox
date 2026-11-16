// The in-memory file tree behind the interactive preview. A path is the same
// virtual path the real app uses ("home/Pictures/a.jpg"); "" is the server
// itself, whose children are the mounts.

export type TextFile = { kind: 'text'; name: string; content: string };
export type ImageFile = { kind: 'image'; name: string; src: string; bytes: number };
export type FileEntry = TextFile | ImageFile;
export type Folder = { kind: 'folder'; name: string; children: Entry[] };
export type Entry = Folder | FileEntry;
export type Located<T extends Entry = Entry> = { path: string; entry: T };

function segments(path: string): string[] {
  return path === '' ? [] : path.split('/');
}

export function joinPath(dir: string, name: string): string {
  return dir === '' ? name : `${dir}/${name}`;
}

export function parentPath(path: string): string {
  return segments(path).slice(0, -1).join('/');
}

export function baseName(path: string): string {
  return segments(path).at(-1) ?? '';
}

export function mountOf(path: string): string {
  return segments(path)[0] ?? '';
}

/** Each folder on the way to `path`, outermost first. */
export function ancestors(path: string): string[] {
  const parts = segments(path);
  return parts.map((_, index) => parts.slice(0, index + 1).join('/'));
}

export function find(root: Folder, path: string): Entry | undefined {
  let node: Entry = root;
  for (const name of segments(path)) {
    if (node.kind !== 'folder') return undefined;
    const next: Entry | undefined = node.children.find((child) => child.name === name);
    if (!next) return undefined;
    node = next;
  }
  return node;
}

export function folderAt(root: Folder, path: string): Folder | undefined {
  const entry = find(root, path);
  return entry?.kind === 'folder' ? entry : undefined;
}

export function sorted(entries: Entry[]): Entry[] {
  return [...entries].sort((a, b) => {
    const foldersFirst = Number(b.kind === 'folder') - Number(a.kind === 'folder');
    return foldersFirst || a.name.localeCompare(b.name);
  });
}

/** `name`, or `name (2)`, `name (3)`… if the folder already has one. */
export function uniqueName(folder: Folder, name: string): string {
  const taken = new Set(folder.children.map((child) => child.name));
  if (!taken.has(name)) return name;
  const dot = name.lastIndexOf('.');
  const stem = dot > 0 ? name.slice(0, dot) : name;
  const extension = dot > 0 ? name.slice(dot) : '';
  for (let count = 2; ; count += 1) {
    const candidate = `${stem} (${count})${extension}`;
    if (!taken.has(candidate)) return candidate;
  }
}

export function entryBytes(entry: Entry): number {
  if (entry.kind === 'folder') return entry.children.reduce((sum, child) => sum + entryBytes(child), 0);
  return entry.kind === 'image' ? entry.bytes : new TextEncoder().encode(entry.content).length;
}

export function formatBytes(bytes: number): string {
  const units = ['B', 'KB', 'MB', 'GB'];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${unit === 0 ? value : Number(value.toFixed(2))} ${units[unit]}`;
}

/** Puts `file` at `relPath` under `dir`, creating the folders in between. */
export function placeFile(root: Folder, dir: string, relPath: string, file: FileEntry): void {
  let folder = folderAt(root, dir);
  for (const name of segments(parentPath(relPath))) {
    if (!folder) return;
    let next: Entry | undefined = folder.children.find((child) => child.name === name);
    if (!next) {
      next = { kind: 'folder', name, children: [] };
      folder.children.push(next);
    }
    folder = next.kind === 'folder' ? next : undefined;
  }
  if (!folder) return;
  folder.children = folder.children.filter((child) => child.name !== file.name);
  folder.children.push(file);
}

export function remove(root: Folder, path: string): void {
  const parent = folderAt(root, parentPath(path));
  if (parent) parent.children = parent.children.filter((child) => child.name !== baseName(path));
}

/** Every file below `folder`, with paths relative to `prefix`. */
export function filesUnder(folder: Folder, prefix = ''): Located<FileEntry>[] {
  return sorted(folder.children).flatMap((child) => {
    const path = joinPath(prefix, child.name);
    return child.kind === 'folder' ? filesUnder(child, path) : [{ path, entry: child }];
  });
}

export function search(root: Folder, query: string): Located[] {
  const needle = query.trim().toLowerCase();
  if (needle === '') return [];
  const matches: Located[] = [];
  const visit = (folder: Folder, dir: string) => {
    for (const child of sorted(folder.children)) {
      const path = joinPath(dir, child.name);
      // Mounts are drives, not search results.
      if (dir !== '' && child.name.toLowerCase().includes(needle)) matches.push({ path, entry: child });
      if (child.kind === 'folder') visit(child, path);
    }
  };
  visit(root, '');
  return matches;
}

// The labels the app's Type column gives the sample's files (frontend/src/lib/utils/fileTypes.ts).
const TYPE_LABELS: Record<string, string> = { jpg: 'JPEG Image', yml: 'YAML', md: 'Markdown', txt: 'Text File', log: 'Log File', conf: 'Config File' };

export function typeOf(entry: Entry): string {
  if (entry.kind === 'folder') return 'Folder';
  const dot = entry.name.lastIndexOf('.');
  if (dot <= 0) return 'File';
  const extension = entry.name.slice(dot + 1).toLowerCase();
  return TYPE_LABELS[extension] ?? `${extension.toUpperCase()} File`;
}
