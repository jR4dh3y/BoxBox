// The interactive preview of BoxBox on the landing page: a working file
// manager over a sample tree. DemoApp.astro renders the chrome; this file owns
// the state and re-renders the parts that change.

import { activeTheme, onThemeChange, pinTheme, themes } from '../siteTheme';
import { el, fill, iconButton, iconsFrom, part, type Icon } from './dom';
import { downloadFile } from './download';
import { ancestors, baseName, entryBytes, filesUnder, find, folderAt, formatBytes, joinPath, mountOf, parentPath, placeFile, remove, search, sorted, typeOf, uniqueName, type Entry, type FileEntry, type Located } from './fs';
import { back, canGoBack, canGoForward, currentPath, forward, visit, type History } from './history';
import { drives } from './drives';
import { contextMenu, editorOverlay, previewOverlay, shareOverlay, themePicker, type Editor, type EditorStatus, type MenuEntry, type MenuItem } from './overlays';
import { picture } from './pictures';
import { createSampleFs, readOnlyMounts } from './sample';
import { sharePage } from './sharePage';
import { newToken, parseAccess, parseExpiry, type ShareLink } from './shares';
import { advance, progressOf, uploadBytes, type Upload, type UploadItem } from './upload';

/** A state the walkthrough can put the preview in. */
export type Scene = 'browse' | 'editor' | 'preview' | 'share' | 'share-page';

export interface DemoApp {
  show(scene: Scene): void;
  /** Uploads a sample folder into Pictures, as a visitor's first impression. */
  startUpload(): void;
}

type Overlay =
  | { kind: 'none' }
  | { kind: 'editor'; path: string; draft: string; status: EditorStatus }
  | { kind: 'preview'; path: string; zoom: number }
  | { kind: 'share'; path: string; copied: string | null }
  | { kind: 'menu'; path: string; x: number; y: number }
  | { kind: 'add'; x: number; y: number }
  | { kind: 'themes' };

type Surface = { kind: 'app' } | { kind: 'guest'; token: string; open: number };

type Toast = { kind: 'none' } | { kind: 'uploading'; upload: Upload } | { kind: 'uploaded'; label: string; count: number; path: string };

const PICTURES = 'home/Pictures';
const SHARED = 'home/Pictures/2026';
const UPLOAD_TICK_MS = 150;
const UPLOAD_TICKS = 30;
const UPLOADED_NOTICE_MS = 7000;
const SAMPLE_FOLDER = ['summer-photos/2026/sunset.jpg', 'summer-photos/2026/harbor.jpg', 'summer-photos/2026/coast.jpg'];

export function mountDemoApp(root: HTMLElement): DemoApp {
  const icon: Icon = iconsFrom(root);
  const fs = createSampleFs();

  const appPart = part(root, 'app', HTMLElement);
  const guestPart = part(root, 'guest', HTMLElement);
  const overlayPart = part(root, 'overlay', HTMLElement);
  const toastPart = part(root, 'toast', HTMLElement);
  const crumbsPart = part(root, 'crumbs', HTMLElement);
  const drivesPart = part(root, 'drives', HTMLElement);
  const driveListPart = part(root, 'drive-list', HTMLElement);
  const listingPart = part(root, 'listing', HTMLElement);
  const searchBox = part(root, 'search-box', HTMLElement);
  const statusPart = part(root, 'status', HTMLElement);
  const selectionPart = part(root, 'selection', HTMLElement);
  const favoritesPart = part(root, 'favorites', HTMLElement);
  const searchInput = part(root, 'search', HTMLInputElement);

  let history: History = { entries: [''], index: 0 };
  let view: 'grid' | 'list' = 'grid';
  let selected: string | null = null;
  let query = '';
  let overlay: Overlay = { kind: 'none' };
  let surface: Surface = { kind: 'app' };
  let toast: Toast = { kind: 'none' };
  let favorites: string[] = [];
  let links: ShareLink[] = [{ token: 'k3v9x2m8qa', path: SHARED, access: 'view', expires: 'week', created: Date.now() - 3 * 60_000 }];
  let editor: Editor | undefined;
  let toastTimer: number | undefined;

  const location = () => currentPath(history);
  const writable = (path: string) => path !== '' && !readOnlyMounts.has(mountOf(path));

  function listed(): Located[] {
    if (query.trim() !== '') return search(fs, query);
    const folder = folderAt(fs, location());
    return folder ? sorted(folder.children).map((entry) => ({ path: joinPath(location(), entry.name), entry })) : [];
  }

  // ---- the file list -------------------------------------------------------

  function thumbnail(entry: Entry, size: string): Node {
    if (entry.kind === 'image') return el('img', { src: entry.src, alt: '', draggable: 'false', class: 'h-full w-full object-cover' });
    return icon(entry.kind === 'folder' ? 'folder' : 'file-text', `${size} ${entry.kind === 'folder' ? 'text-accent' : 'text-text-secondary'}`);
  }

  function detail(item: Located): string {
    if (query.trim() !== '') return parentPath(item.path);
    return item.entry.kind === 'folder' ? 'Folder' : formatBytes(entryBytes(item.entry));
  }

  function tile(item: Located): HTMLElement {
    const folder = item.entry.kind === 'folder';
    return el(
      'button',
      { type: 'button', 'data-action': 'entry', 'data-path': item.path, 'aria-pressed': String(selected === item.path), title: item.entry.name, class: 'flex min-w-0 flex-col items-center gap-1 rounded-md p-1.5 text-center hover:bg-surface-secondary/70 aria-pressed:bg-selection/80' },
      el('span', { class: 'flex aspect-[8/5] w-full items-center justify-center overflow-hidden rounded border border-border-primary bg-surface-secondary/85' }, thumbnail(item.entry, 'h-7 w-7')),
      el('span', { class: `w-full truncate text-[12px] ${folder ? 'text-accent' : 'text-text-primary'}` }, item.entry.name),
      el('span', { class: 'w-full truncate text-[10px] text-text-muted' }, detail(item))
    );
  }

  // The app's list columns (FileList.svelte), less Modified: the sample tree has no dates.
  const listColumns = 'grid grid-cols-[minmax(0,1fr)_28%_18%]';

  function listHeader(): HTMLElement {
    const searching = query.trim() !== '';
    return el(
      'div',
      { class: `${listColumns} sticky top-0 z-[5] h-8 items-center border-b border-border-primary bg-surface-secondary/90 text-[13px] font-medium whitespace-nowrap text-text-secondary backdrop-blur-md select-none` },
      el('span', { class: 'px-3' }, 'Name'),
      el('span', { class: 'px-3' }, searching ? 'Location' : 'Type'),
      el('span', { class: 'px-3 text-right' }, 'Size')
    );
  }

  function row(item: Located): HTMLElement {
    const searching = query.trim() !== '';
    const second = searching ? parentPath(item.path) : typeOf(item.entry);
    const size = item.entry.kind === 'folder' ? '—' : formatBytes(entryBytes(item.entry));
    return el(
      'button',
      { type: 'button', 'data-action': 'entry', 'data-path': item.path, 'aria-pressed': String(selected === item.path), class: `${listColumns} h-8 w-full items-center text-left text-[13px] transition-colors duration-50 hover:bg-surface-secondary/70 aria-pressed:bg-selection aria-pressed:hover:bg-selection-hover` },
      el('span', { class: 'flex min-w-0 items-center gap-2 px-3' }, el('span', { class: 'flex h-4 w-4 shrink-0 items-center justify-center overflow-hidden rounded-sm' }, thumbnail(item.entry, 'h-4 w-4')), el('span', { class: 'truncate text-text-primary' }, item.entry.name)),
      el('span', { class: 'truncate px-3 text-text-secondary in-aria-pressed:text-text-primary' }, second),
      el('span', { class: 'truncate px-3 text-right text-text-secondary tabular-nums in-aria-pressed:text-text-primary' }, size)
    );
  }

  function renderListing() {
    const atServer = location() === '' && query.trim() === '';
    drivesPart.hidden = !atServer || view === 'list';
    driveListPart.hidden = !atServer || view === 'grid';
    listingPart.hidden = atServer;
    if (atServer) return;
    const items = listed();
    if (items.length === 0) {
      listingPart.className = 'flex h-full items-center justify-center text-text-muted';
      listingPart.replaceChildren(query.trim() === '' ? 'This folder is empty.' : `Nothing matches “${query.trim()}”.`);
      return;
    }
    if (view === 'grid') {
      listingPart.className = 'grid grid-cols-[repeat(auto-fill,minmax(100px,1fr))] content-start gap-2 p-3';
      listingPart.replaceChildren(...items.map(tile));
    } else {
      listingPart.className = 'flex min-w-0 flex-col';
      listingPart.replaceChildren(listHeader(), ...items.map(row));
    }
  }

  // ---- the chrome around it ------------------------------------------------

  function setDisabled(action: string, disabled: boolean) {
    root.querySelectorAll<HTMLButtonElement>(`button[data-action="${action}"]`).forEach((button) => {
      button.disabled = disabled;
    });
  }

  function renderChrome() {
    const path = location();
    const crumb = (label: string, target: string, last: boolean) =>
      el('button', { type: 'button', 'data-action': 'go', 'data-path': target, class: last ? 'truncate text-text-primary' : 'shrink-0 text-text-secondary hover:text-white hover:underline' }, label);
    const trail = ancestors(path);
    fill(
      crumbsPart,
      ...(trail.length === 0
        ? [crumb('This Server', '', true)]
        : trail.flatMap((target, index) => [index > 0 && el('span', { class: 'text-xs text-text-muted' }, '/'), crumb(baseName(target), target, index === trail.length - 1)]))
    );

    setDisabled('back', !canGoBack(history));
    setDisabled('forward', !canGoForward(history));
    setDisabled('add', !writable(path));
    searchBox.hidden = path === '';

    root.querySelectorAll<HTMLElement>('[data-nav]').forEach((item) => item.toggleAttribute('data-active', item.dataset.path === path));
    root.querySelectorAll<HTMLElement>('[data-view]').forEach((button) => button.setAttribute('aria-pressed', String(button.dataset.view === view)));

    favoritesPart.replaceChildren(
      ...(favorites.length === 0
        ? [el('div', { class: 'px-5 py-2 text-xs text-text-muted italic' }, 'No favorites yet')]
        : favorites.map((favorite) =>
            el(
              'button',
              { type: 'button', 'data-action': 'go', 'data-path': favorite, class: `flex w-full items-center gap-2.5 px-3 py-1.5 pl-5 text-left ${favorite === path ? 'bg-selection text-white' : 'text-text-primary hover:bg-surface-secondary'}` },
              icon('star', 'h-4 w-4 opacity-80'),
              el('span', { class: 'flex-1 truncate' }, baseName(favorite))
            )
          ))
    );
  }

  function renderStatus() {
    const count = location() === '' && query.trim() === '' ? drives.length : listed().length;
    const noun = query.trim() === '' ? 'item' : 'result';
    fill(
      statusPart,
      selected ? `1 of ${count} selected` : `${count} ${noun}${count === 1 ? '' : 's'}`,
      !writable(location()) && location() !== '' && el('span', { class: 'ml-3 inline-flex items-center gap-1 text-text-muted' }, icon('lock', 'h-3 w-3'), 'Read-only')
    );
    selectionPart.replaceChildren(...(selected ? menuItems(selected).map((item) => iconButton(icon(item.icon, 'h-3.5 w-3.5'), item.label, { 'data-action': item.action, 'data-path': selected ?? '', class: 'flex h-6 w-6 items-center justify-center rounded text-text-secondary hover:bg-surface-elevated hover:text-text-primary' })) : []));
  }

  /** The toolbar's "+" menu, as in the app's AddMenu. */
  function addItems(): MenuEntry[] {
    const canCreate = writable(location());
    const canUpload = canCreate && toast.kind !== 'uploading';
    return [
      { label: 'New folder', action: 'new-folder', icon: 'folder-plus', disabled: !canCreate },
      { label: 'New file', action: 'new-file', icon: 'file-plus', disabled: !canCreate },
      'separator',
      { label: 'Upload files', action: 'upload-files', icon: 'upload', disabled: !canUpload },
      { label: 'Upload folder', action: 'upload-folder', icon: 'folder-up', disabled: !canUpload }
    ];
  }

  function menuItems(path: string): MenuItem[] {
    const entry = find(fs, path);
    if (!entry) return [];
    return [
      { label: 'Open', action: 'open', icon: entry.kind === 'folder' ? 'folder' : 'file-text' },
      { label: 'Share', action: 'share', icon: 'share' },
      ...(entry.kind === 'folder' ? [{ label: favorites.includes(path) ? 'Remove from favorites' : 'Add to favorites', action: 'favorite', icon: 'star' } satisfies MenuItem] : []),
      ...(writable(path) ? [{ label: 'Delete', action: 'delete', icon: 'trash' } satisfies MenuItem] : [])
    ];
  }

  // ---- overlays and the guest page ----------------------------------------

  function imagesBeside(path: string): string[] {
    const folder = folderAt(fs, parentPath(path));
    return folder ? sorted(folder.children).filter((entry) => entry.kind === 'image').map((entry) => joinPath(parentPath(path), entry.name)) : [];
  }

  function renderOverlay() {
    const opening = overlayPart.hidden;
    editor = undefined;
    const entry = 'path' in overlay ? find(fs, overlay.path) : undefined;
    let nodes: Node[] = [];
    if (overlay.kind === 'editor' && entry?.kind === 'text') {
      editor = editorOverlay(icon, { name: entry.name, content: overlay.draft, readOnly: !writable(overlay.path) });
      editor.refresh(overlay.draft, overlay.status);
      nodes = editor.nodes;
    } else if (overlay.kind === 'preview' && entry?.kind === 'image') {
      nodes = previewOverlay(icon, entry, overlay.zoom, imagesBeside(overlay.path).length > 1);
    } else if (overlay.kind === 'share' && entry) {
      const path = overlay.path;
      nodes = shareOverlay(icon, entry.name, links.filter((link) => link.path === path), overlay.copied, Date.now());
    } else if (overlay.kind === 'menu' && entry) {
      nodes = contextMenu(icon, overlay.x, overlay.y, menuItems(overlay.path));
    } else if (overlay.kind === 'add') {
      nodes = contextMenu(icon, overlay.x, overlay.y, addItems());
    } else if (overlay.kind === 'themes') {
      nodes = themePicker(themes, activeTheme());
    }
    overlayPart.hidden = nodes.length === 0;
    overlayPart.replaceChildren(...nodes);
    // Keyboard users follow an overlay they opened. The walkthrough opens
    // them on a timer too, and must not pull focus in from the page.
    if (opening && !overlayPart.hidden && root.contains(document.activeElement)) {
      overlayPart.querySelector<HTMLElement>('button:not([disabled]), textarea, input, select, [tabindex]:not([tabindex="-1"])')?.focus({ preventScroll: true });
    }
    appPart.inert = !overlayPart.hidden || !guestPart.hidden;
  }

  function guestFiles(link: ShareLink): Located<FileEntry>[] {
    const entry = find(fs, link.path);
    if (!entry) return [];
    return entry.kind === 'folder' ? filesUnder(entry) : [{ path: entry.name, entry }];
  }

  function openLink(): ShareLink | undefined {
    const current = surface;
    return current.kind === 'guest' ? links.find((link) => link.token === current.token) : undefined;
  }

  function renderSurface() {
    const link = openLink();
    if (surface.kind !== 'guest' || !link) {
      surface = { kind: 'app' };
      guestPart.hidden = true;
      guestPart.replaceChildren();
      appPart.inert = !overlayPart.hidden;
      return;
    }
    guestPart.hidden = false;
    appPart.inert = true;
    guestPart.replaceChildren(...sharePage(icon, { name: baseName(link.path), link, files: guestFiles(link), open: surface.open }));
  }

  function render() {
    renderChrome();
    renderListing();
    renderStatus();
    renderOverlay();
    renderSurface();
  }

  // Re-rendering replaces the button that was just used. Focus goes back to
  // its replacement, or to the preview itself, so the keyboard keeps working.
  function keepingFocus(draw: () => void) {
    const before = document.activeElement;
    const inside = before instanceof HTMLElement && root.contains(before) ? before : null;
    draw();
    if (!inside || root.contains(document.activeElement)) return;
    const scope = overlayPart.hidden ? root : overlayPart;
    const selector = inside.dataset.action
      ? `[data-action="${inside.dataset.action}"]` + ['path', 'step', 'token', 'index'].map((name) => (inside.dataset[name] === undefined ? '' : `[data-${name}="${CSS.escape(inside.dataset[name])}"]`)).join('')
      : null;
    const again = selector ? scope.querySelector<HTMLElement>(selector) : null;
    (again ?? root).focus({ preventScroll: true });
  }

  // ---- uploads -------------------------------------------------------------

  function renderToast() {
    // One handle serves both the upload's ticks and the notice's timeout.
    window.clearInterval(toastTimer);
    window.clearTimeout(toastTimer);
    toastPart.hidden = toast.kind === 'none';
    toastPart.replaceChildren();
    if (toast.kind === 'none') return;

    const title = el('span', { class: 'flex min-w-0 items-center gap-2 text-text-primary' });
    const close = el('button', { type: 'button', 'data-action': 'toast-close', 'aria-label': 'Dismiss', class: 'text-text-muted hover:text-text-primary' }, icon('x', 'h-3.5 w-3.5'));
    const body = el('div', { class: 'flex flex-col gap-2.5 p-4' });
    toastPart.append(el('div', { class: 'flex items-center justify-between gap-3 border-b border-border-primary bg-surface-tertiary/95 px-4 py-2 text-[12px] font-medium' }, title, close), body);

    if (toast.kind === 'uploaded') {
      title.append(icon('check', 'h-3.5 w-3.5 shrink-0 text-accent'), el('span', { class: 'truncate' }, `Uploaded ${toast.label}`));
      body.append(
        el(
          'div',
          { class: 'flex items-center justify-between gap-3 text-xs text-text-secondary' },
          `${toast.count} ${toast.count === 1 ? 'file' : 'files'}, paths preserved`,
          el('button', { type: 'button', 'data-action': 'go', 'data-path': toast.path, class: 'shrink-0 font-medium text-accent hover:text-white' }, 'Show')
        )
      );
      toastTimer = window.setTimeout(() => keepingFocus(dismissToast), UPLOADED_NOTICE_MS);
      return;
    }

    const name = el('span', { class: 'truncate pr-4 font-medium text-text-primary' });
    const bytes = el('span', { class: 'shrink-0 font-mono text-[10px] text-text-secondary' });
    const bar = el('div', { class: 'h-full rounded-full bg-accent transition-[width] duration-150' });
    const count = el('span');
    title.append(icon('loader', 'h-3.5 w-3.5 shrink-0 animate-spin text-accent'), el('span', { class: 'truncate' }, `Uploading ${toast.upload.label}...`));
    body.append(
      el('div', { class: 'flex items-center justify-between text-xs' }, name, bytes),
      el('div', { class: 'h-1.5 w-full overflow-hidden rounded-full bg-surface-elevated' }, bar),
      el('div', { class: 'flex justify-between font-mono text-[10px] text-text-muted' }, count, el('span', {}, 'Paths preserved'))
    );

    const paint = (upload: Upload) => {
      const progress = progressOf(upload);
      name.textContent = progress.current?.relPath ?? '';
      bytes.textContent = `${formatBytes(progress.currentSent)} / ${formatBytes(progress.currentBytes)}`;
      bar.style.width = `${progress.percent}%`;
      count.textContent = `${progress.percent}% · ${Math.min(progress.completed + 1, upload.items.length)} of ${upload.items.length} files`;
    };
    paint(toast.upload);

    toastTimer = window.setInterval(() => keepingFocus(() => {
      if (toast.kind !== 'uploading') return;
      const step = advance(toast.upload, Math.ceil(uploadBytes(toast.upload) / UPLOAD_TICKS));
      const target = step.upload.target;
      // The visitor may delete the folder an upload is going into.
      if (!folderAt(fs, target)) {
        dismissToast();
        return;
      }
      for (const item of step.finished) placeFile(fs, target, item.relPath, item.file);
      if (progressOf(step.upload).done) {
        const parent = parentPath(step.upload.items[0]?.relPath ?? '');
        toast = { kind: 'uploaded', label: step.upload.label, count: step.upload.items.length, path: parent ? joinPath(target, parent) : target };
        renderToast();
        // Redrawing an open editor would drop the visitor's caret.
        if (overlay.kind === 'editor') {
          renderChrome();
          renderListing();
          renderStatus();
        } else {
          render();
        }
        return;
      }
      toast = { kind: 'uploading', upload: step.upload };
      paint(step.upload);
      if (step.finished.length > 0) {
        renderListing();
        renderStatus();
      }
    }), UPLOAD_TICK_MS);
  }

  function dismissToast() {
    toast = { kind: 'none' };
    renderToast();
    renderChrome();
  }

  function upload(target: string, label: string, relPaths: string[]) {
    const folder = folderAt(fs, target);
    if (!folder || toast.kind === 'uploading') return;
    // A second upload of the same thing lands beside the first, not on it.
    const top = uniqueName(folder, label);
    const items: UploadItem[] = relPaths.map((relPath) => ({ relPath: relPath.replace(label, top), file: picture(baseName(relPath.replace(label, top))) }));
    toast = { kind: 'uploading', upload: { label: top, target, items, sent: 0 } };
    renderToast();
    renderChrome();
  }

  // ---- actions -------------------------------------------------------------

  function go(path: string) {
    if (!folderAt(fs, path)) return;
    history = visit(history, path);
    arrive();
  }

  function arrive() {
    selected = null;
    query = '';
    searchInput.value = '';
    overlay = { kind: 'none' };
    surface = { kind: 'app' };
    render();
  }

  function open(path: string) {
    const entry = find(fs, path);
    if (!entry) return;
    if (entry.kind === 'folder') {
      go(path);
      return;
    }
    selected = path;
    overlay = entry.kind === 'text' ? { kind: 'editor', path, draft: entry.content, status: 'clean' } : { kind: 'preview', path, zoom: 1 };
    render();
  }

  function forget(path: string) {
    const gone = (candidate: string) => candidate === path || candidate.startsWith(`${path}/`);
    favorites = favorites.filter((favorite) => !gone(favorite));
    links = links.filter((link) => !gone(link.path));
  }

  function setOverlay(next: Overlay) {
    overlay = next;
    renderOverlay();
  }

  /** The path an action is about: its own, or the open overlay's. */
  function subject(target: HTMLElement): string | null {
    return target.dataset.path ?? ('path' in overlay ? overlay.path : selected);
  }

  const actions: Record<string, (target: HTMLElement, event: MouseEvent) => void> = {
    go: (target) => go(target.dataset.path ?? ''),
    back: () => {
      history = back(history);
      arrive();
    },
    forward: () => {
      history = forward(history);
      arrive();
    },
    view: (target) => {
      view = target.dataset.view === 'list' ? 'list' : 'grid';
      renderChrome();
      renderListing();
    },
    section: (target) => {
      const open = target.getAttribute('aria-expanded') !== 'true';
      target.setAttribute('aria-expanded', String(open));
      const body = target.nextElementSibling;
      if (body instanceof HTMLElement) body.hidden = !open;
    },
    entry: (target, event) => {
      const path = target.dataset.path ?? '';
      // A second click opens, as a double click would; a finger opens at once.
      const touch = event instanceof PointerEvent && event.pointerType === 'touch';
      if (selected === path || touch) {
        open(path);
        return;
      }
      selected = path;
      renderListing();
      renderStatus();
    },
    open: (target) => {
      const path = subject(target);
      if (path) open(path);
    },
    share: (target) => {
      const path = subject(target);
      if (path) setOverlay({ kind: 'share', path, copied: null });
    },
    favorite: (target) => {
      const path = subject(target);
      if (!path) return;
      favorites = favorites.includes(path) ? favorites.filter((favorite) => favorite !== path) : [...favorites, path];
      overlay = { kind: 'none' };
      render();
    },
    delete: (target) => {
      const path = subject(target);
      if (!path || !writable(path)) return;
      remove(fs, path);
      forget(path);
      selected = null;
      overlay = { kind: 'none' };
      render();
    },
    close: () => setOverlay({ kind: 'none' }),
    save: () => {
      if (overlay.kind !== 'editor') return;
      const entry = find(fs, overlay.path);
      if (entry?.kind !== 'text') return;
      entry.content = overlay.draft;
      overlay = { ...overlay, status: 'saved' };
      editor?.refresh(overlay.draft, overlay.status);
      renderListing();
    },
    download: () => {
      const guest = openLink();
      const entry = guest && surface.kind === 'guest' ? guestFiles(guest)[surface.open]?.entry : 'path' in overlay ? find(fs, overlay.path) : undefined;
      if (entry && entry.kind !== 'folder') downloadFile(entry);
    },
    'preview-step': (target) => {
      if (overlay.kind !== 'preview') return;
      const images = imagesBeside(overlay.path);
      const next = images[(images.indexOf(overlay.path) + Number(target.dataset.step) + images.length) % images.length];
      if (!next) return;
      selected = next;
      overlay = { kind: 'preview', path: next, zoom: 1 };
      render();
    },
    'zoom-in': () => {
      if (overlay.kind === 'preview') setOverlay({ ...overlay, zoom: Math.min(overlay.zoom + 0.25, 3) });
    },
    'zoom-out': () => {
      if (overlay.kind === 'preview') setOverlay({ ...overlay, zoom: Math.max(overlay.zoom - 0.25, 0.5) });
    },
    'zoom-reset': () => {
      if (overlay.kind === 'preview') setOverlay({ ...overlay, zoom: 1 });
    },
    'share-create': () => {
      if (overlay.kind !== 'share') return;
      const access = overlayPart.querySelector<HTMLSelectElement>('select[name="access"]')?.value ?? '';
      const expires = overlayPart.querySelector<HTMLSelectElement>('select[name="expires"]')?.value ?? '';
      links = [...links, { token: newToken(), path: overlay.path, access: parseAccess(access), expires: parseExpiry(expires), created: Date.now() }];
      renderOverlay();
    },
    'share-revoke': (target) => {
      links = links.filter((link) => link.token !== target.dataset.token);
      renderOverlay();
    },
    'share-copy': (target) => {
      if (overlay.kind !== 'share') return;
      const token = target.dataset.token ?? '';
      // The clipboard can be refused; the tick still shows what would happen.
      navigator.clipboard?.writeText(`https://files.example.com/s/${token}`).catch(() => undefined);
      setOverlay({ ...overlay, copied: token });
    },
    'share-open': (target) => {
      overlay = { kind: 'none' };
      surface = { kind: 'guest', token: target.dataset.token ?? '', open: 0 };
      render();
    },
    'share-file': (target) => {
      if (surface.kind !== 'guest') return;
      surface = { ...surface, open: Number(target.dataset.index) };
      renderSurface();
    },
    'share-step': (target) => {
      if (surface.kind !== 'guest') return;
      const link = openLink();
      const total = link ? guestFiles(link).length : 0;
      if (total === 0) return;
      surface = { ...surface, open: (surface.open + Number(target.dataset.step) + total) % total };
      renderSurface();
    },
    'share-exit': () => {
      surface = { kind: 'app' };
      renderSurface();
    },
    themes: () => setOverlay(overlay.kind === 'themes' ? { kind: 'none' } : { kind: 'themes' }),
    theme: (target) => {
      const theme = themes[Number(target.dataset.index)];
      if (theme) pinTheme(theme);
    },
    refresh: (target) => {
      target.firstElementChild?.animate({ transform: ['rotate(0)', 'rotate(360deg)'] }, { duration: 500, easing: 'ease-out' });
      render();
    },
    add: (target) => {
      if (overlay.kind === 'add') {
        setOverlay({ kind: 'none' });
        return;
      }
      // The preview may be drawn scaled; the menu is placed in its own pixels.
      const box = root.getBoundingClientRect();
      const button = target.getBoundingClientRect();
      const scale = box.width / root.offsetWidth || 1;
      setOverlay({ kind: 'add', x: Math.min((button.left - box.left) / scale, root.offsetWidth - 184), y: (button.bottom - box.top) / scale + 4 });
    },
    'new-folder': () => {
      const folder = folderAt(fs, location());
      if (!folder || !writable(location())) return;
      const name = uniqueName(folder, 'New folder');
      folder.children.push({ kind: 'folder', name, children: [] });
      selected = joinPath(location(), name);
      // A search would hide the folder that was just made and selected.
      query = '';
      searchInput.value = '';
      overlay = { kind: 'none' };
      render();
    },
    'new-file': () => {
      const folder = folderAt(fs, location());
      if (!folder || !writable(location())) return;
      const name = uniqueName(folder, 'untitled.txt');
      folder.children.push({ kind: 'text', name, content: '' });
      open(joinPath(location(), name));
      editor?.textarea.focus();
    },
    'upload-files': () => {
      overlay = { kind: 'none' };
      renderOverlay();
      upload(location(), 'IMG_2041.jpg', ['IMG_2041.jpg']);
    },
    'upload-folder': () => {
      overlay = { kind: 'none' };
      renderOverlay();
      upload(location(), 'summer-photos', SAMPLE_FOLDER);
    },
    'toast-close': dismissToast
  };

  root.addEventListener('click', (event) => {
    const target = event.target instanceof Element ? event.target.closest<HTMLElement>('[data-action]') : null;
    const action = target && root.contains(target) ? actions[target.dataset.action ?? ''] : undefined;
    if (target && action) keepingFocus(() => action(target, event));
  });

  root.addEventListener('contextmenu', (event) => {
    const target = event.target instanceof Element ? event.target.closest<HTMLElement>('[data-action="entry"]') : null;
    const path = target?.dataset.path;
    if (!path) return;
    event.preventDefault();
    // The preview may be drawn scaled; the menu is placed in its own pixels.
    const box = root.getBoundingClientRect();
    const scale = box.width / root.offsetWidth || 1;
    selected = path;
    overlay = {
      kind: 'menu',
      path,
      x: Math.min((event.clientX - box.left) / scale, root.offsetWidth - 184),
      y: Math.min((event.clientY - box.top) / scale, root.offsetHeight - 140)
    };
    keepingFocus(render);
  });

  root.addEventListener('input', (event) => {
    if (event.target === searchInput) {
      query = searchInput.value;
      selected = null;
      renderListing();
      renderStatus();
    } else if (overlay.kind === 'editor' && editor && event.target === editor.textarea) {
      overlay = { ...overlay, draft: editor.textarea.value, status: 'dirty' };
      editor.refresh(overlay.draft, overlay.status);
    }
  });

  root.addEventListener('keydown', (event) => {
    if (event.key !== 'Escape' || overlay.kind === 'none') return;
    event.stopPropagation();
    keepingFocus(() => setOverlay({ kind: 'none' }));
  });

  onThemeChange(() => {
    if (overlay.kind === 'themes') renderOverlay();
  });

  render();

  return {
    show(scene) {
      const jump = (path: string) => {
        if (folderAt(fs, path)) history = visit(history, path);
        selected = null;
        query = '';
        searchInput.value = '';
        overlay = { kind: 'none' };
        surface = { kind: 'app' };
      };
      if (scene === 'editor') {
        jump('home/Documents');
        open('home/Documents/docker-compose.yml');
      } else {
        jump(PICTURES);
        view = 'grid';
        if (scene === 'preview') open(`${PICTURES}/sunset.jpg`);
        else if (scene !== 'browse' && find(fs, SHARED)) {
          selected = SHARED;
          const link = links.find((candidate) => candidate.path === SHARED);
          if (scene === 'share') overlay = { kind: 'share', path: SHARED, copied: null };
          else if (link) surface = { kind: 'guest', token: link.token, open: guestFiles(link).length - 1 };
        }
      }
      render();
    },
    startUpload() {
      upload(PICTURES, 'summer-photos', SAMPLE_FOLDER);
    }
  };
}
