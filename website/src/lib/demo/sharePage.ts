// What a guest sees when they open a share link: no sidebar, no sign-in, only
// the shared files and the access the link grants.

import { el, iconButton, type Icon } from './dom';
import { entryBytes, formatBytes, type FileEntry, type Located } from './fs';
import { accessLabels, expirySentence, type ShareLink } from './shares';

export type SharePage = { name: string; link: ShareLink; files: Located<FileEntry>[]; open: number };

function viewer(file: FileEntry): HTMLElement {
  if (file.kind === 'image') return el('img', { src: file.src, alt: file.name, draggable: 'false', class: 'max-h-full max-w-full' });
  return el('pre', { class: 'mock-scrollbar h-full w-full overflow-auto p-4 font-mono text-[12px] leading-5 text-text-primary' }, file.content);
}

export function sharePage(icon: Icon, page: SharePage): Node[] {
  const current = page.files[page.open];
  const several = page.files.length > 1;
  const crumbs = [page.name, ...(current && current.path !== page.name ? current.path.split('/') : [])];
  const bar = 'rounded-lg border border-white/10 bg-[#1a1d24]/90';

  return [
    el(
      'div',
      { class: 'flex shrink-0 items-center gap-3 bg-accent px-3 py-1.5 text-[12px] font-medium text-[#0d0d0d]' },
      el('span', { class: 'mr-auto truncate' }, 'This is the guest view. No account, no sign-in.'),
      el('button', { type: 'button', 'data-action': 'share-exit', class: 'shrink-0 rounded border border-[#0d0d0d]/40 px-2 py-0.5 hover:bg-[#0d0d0d] hover:text-accent' }, 'Back to my files')
    ),
    el(
      'div',
      { class: `m-2 mb-0 flex h-11 shrink-0 items-center gap-2 px-3 ${bar}` },
      el('span', { class: 'font-display text-[15px] font-extrabold tracking-tight text-accent' }, 'BoxBox'),
      el(
        'div',
        { class: 'flex h-7 min-w-0 flex-1 items-center gap-1.5 rounded border border-white/10 bg-black/30 px-2 text-text-secondary' },
        ...crumbs.flatMap((crumb, index) => [
          index > 0 && el('span', { class: 'text-text-muted' }, '/'),
          el('span', { class: index === crumbs.length - 1 ? 'truncate text-white' : 'shrink-0' }, crumb)
        ])
      ),
      several && iconButton(icon('chevron-left'), 'Previous file', { 'data-action': 'share-step', 'data-step': '-1' }),
      several && iconButton(icon('chevron-right'), 'Next file', { 'data-action': 'share-step', 'data-step': '1' }),
      page.link.access === 'download' && current && iconButton(icon('download'), 'Download', { 'data-action': 'download' })
    ),
    el(
      'div',
      { class: 'flex min-h-0 flex-1 gap-2 p-2' },
      el(
        'div',
        { class: `hidden w-52 shrink-0 flex-col overflow-hidden @2xl:flex ${bar}` },
        el(
          'div',
          { class: 'mock-scrollbar flex-1 overflow-y-auto' },
          ...page.files.map((file, index) =>
            el(
              'button',
              { type: 'button', 'data-action': 'share-file', 'data-index': String(index), 'aria-pressed': String(index === page.open), class: 'flex w-full items-center gap-2 px-3 py-1.5 text-left hover:bg-white/5 aria-pressed:bg-selection aria-pressed:text-white' },
              icon('file-text', 'h-3.5 w-3.5 shrink-0 opacity-70'),
              el('span', { class: 'flex-1 truncate' }, file.entry.name),
              el('span', { class: 'shrink-0 text-[10px] text-text-muted' }, formatBytes(entryBytes(file.entry)))
            )
          )
        ),
        el('div', { class: 'shrink-0 border-t border-white/10 px-3 py-2 text-[11px] text-text-muted' }, `${accessLabels[page.link.access]} · ${expirySentence(page.link.expires)}`)
      ),
      el(
        'div',
        { class: `flex min-w-0 flex-1 items-center justify-center overflow-hidden ${bar}` },
        current ? viewer(current.entry) : el('p', { class: 'text-text-muted' }, 'Nothing has been shared here yet.')
      )
    )
  ];
}
