// Everything the preview opens on top of the file list: the editor, the image
// preview, the share dialog, the context menu and the theme picker. Each
// builder returns nodes; buttons carry a data-action that app.ts handles.

import { el, iconButton, type Child, type Icon, type IconName } from './dom';
import { formatBytes, type ImageFile } from './fs';
import { isYaml, tokenizeYamlLine, type Token } from './highlight';
import { accessLabels, createdLabel, expiryLabels, type ShareLink } from './shares';
import type { Theme } from '../siteTheme';

export type EditorStatus = 'clean' | 'dirty' | 'saved';

const panel = 'demo-pop relative flex flex-col overflow-hidden rounded-lg border border-border-primary bg-surface-primary shadow-2xl';
const select = 'h-9 w-full rounded border border-border-primary bg-surface-secondary px-2 text-text-primary';

function backdrop(className: string): HTMLButtonElement {
  return el('button', { type: 'button', 'data-action': 'close', 'aria-label': 'Close', tabindex: '-1', class: `absolute inset-0 cursor-default ${className}` });
}

function header(icon: Icon, title: string, detail: string, ...actions: Child[]): HTMLElement {
  return el(
    'div',
    { class: 'flex h-12 shrink-0 items-center gap-2 border-b border-border-secondary px-4' },
    el('span', { class: 'truncate text-[14px] font-medium text-white' }, title),
    el('span', { class: 'mr-auto shrink-0 text-[11px] text-text-muted' }, detail),
    ...actions,
    iconButton(icon('x'), 'Close', { 'data-action': 'close' })
  );
}

const tokenClasses: Record<Token['kind'], string> = {
  plain: 'text-text-primary',
  key: 'text-[#4ec9b0]',
  value: 'text-[#ce9178]',
  comment: 'text-text-muted'
};

export type Editor = { nodes: Node[]; textarea: HTMLTextAreaElement; refresh(draft: string, status: EditorStatus): void };

export function editorOverlay(icon: Icon, file: { name: string; content: string; readOnly: boolean }): Editor {
  const size = el('span', { class: 'mr-auto shrink-0 text-[11px] text-text-muted' });
  const state = el('span', { class: 'text-text-muted' });
  const save = el('button', { type: 'button', 'data-action': 'save', class: 'ml-auto rounded border border-border-primary px-3 py-1 text-text-primary transition-colors enabled:hover:border-accent enabled:hover:text-accent disabled:text-text-disabled' }, 'Save');
  const gutter = el('div', { 'aria-hidden': 'true', class: 'sticky left-0 z-10 select-none bg-[#1b1b1b] pr-4 pl-4 text-right text-text-muted' });
  const code = el('div', { 'aria-hidden': 'true', class: 'pr-6 whitespace-pre' });
  const textarea = el('textarea', {
    'aria-label': `Contents of ${file.name}`,
    spellcheck: 'false',
    wrap: 'off',
    readonly: file.readOnly,
    class: 'absolute inset-0 h-full w-full resize-none overflow-hidden bg-transparent pr-6 whitespace-pre text-transparent caret-white outline-none'
  });
  textarea.value = file.content;
  const colour = isYaml(file.name);

  function refresh(draft: string, status: EditorStatus) {
    const lines = draft.split('\n');
    gutter.replaceChildren(...lines.map((_, index) => el('div', {}, String(index + 1))));
    code.replaceChildren(
      ...lines.map((line) => {
        const tokens: Token[] = colour ? tokenizeYamlLine(line) : [{ text: line, kind: 'plain' }];
        return el('div', { class: 'h-5' }, ...tokens.map((token) => el('span', { class: tokenClasses[token.kind] }, token.text)));
      })
    );
    size.textContent = formatBytes(new TextEncoder().encode(draft).length);
    state.textContent = status === 'dirty' ? 'Unsaved changes' : status === 'saved' ? 'Saved' : 'No changes';
    save.disabled = status !== 'dirty';
  }
  refresh(file.content, 'clean');

  const nodes = [
    backdrop('bg-black/60'),
    el(
      'div',
      { role: 'dialog', 'aria-label': `Editing ${file.name}`, class: `${panel} h-full w-full` },
      el(
        'div',
        { class: 'flex h-12 shrink-0 items-center gap-2 border-b border-border-secondary px-4' },
        el('span', { class: 'truncate text-[14px] font-medium text-white' }, file.name),
        size,
        iconButton(icon('download'), 'Download', { 'data-action': 'download' }),
        iconButton(icon('x'), 'Close', { 'data-action': 'close' })
      ),
      el(
        'div',
        { class: 'flex h-10 shrink-0 items-center gap-3 border-b border-border-secondary px-4 text-[12px]' },
        el('span', { class: 'flex items-center gap-1.5 text-white' }, file.readOnly && icon('lock', 'h-3 w-3'), file.readOnly ? 'Read-only' : 'Editable'),
        state,
        !file.readOnly && save
      ),
      el(
        'div',
        { class: 'mock-scrollbar flex-1 overflow-auto bg-[#1b1b1b] py-2 font-mono text-[12px] leading-5 [tab-size:4]' },
        el('div', { class: 'flex min-h-full min-w-max' }, gutter, el('div', { class: 'relative flex-1' }, code, textarea))
      )
    )
  ];
  return { nodes, textarea, refresh };
}

export function previewOverlay(icon: Icon, file: ImageFile, zoom: number, siblings: boolean): Node[] {
  const zoomButton = (name: IconName, label: string, action: string) => iconButton(icon(name, 'h-3.5 w-3.5'), label, { 'data-action': action });
  return [
    backdrop('bg-black/60'),
    el(
      'div',
      { role: 'dialog', 'aria-label': `Preview of ${file.name}`, class: `${panel} h-full w-full` },
      header(
        icon,
        file.name,
        formatBytes(file.bytes),
        siblings && iconButton(icon('chevron-left'), 'Previous image', { 'data-action': 'preview-step', 'data-step': '-1' }),
        siblings && iconButton(icon('chevron-right'), 'Next image', { 'data-action': 'preview-step', 'data-step': '1' }),
        iconButton(icon('download'), 'Download', { 'data-action': 'download' })
      ),
      el(
        'div',
        { class: 'flex h-9 shrink-0 items-center justify-center gap-1 border-b border-border-secondary text-[11px] text-text-secondary' },
        zoomButton('zoom-out', 'Zoom out', 'zoom-out'),
        el('span', { class: 'w-10 text-center tabular-nums' }, `${Math.round(zoom * 100)}%`),
        zoomButton('zoom-in', 'Zoom in', 'zoom-in'),
        zoomButton('rotate-ccw', 'Reset zoom', 'zoom-reset')
      ),
      el(
        'div',
        { class: 'flex min-h-0 flex-1 items-center justify-center overflow-hidden bg-[#141414] p-3' },
        el('img', { src: file.src, alt: file.name, draggable: 'false', style: `transform: scale(${zoom})`, class: 'max-h-full max-w-full transition-transform duration-200' })
      )
    )
  ];
}

function options(labels: Record<string, string>): HTMLOptionElement[] {
  return Object.entries(labels).map(([value, label]) => el('option', { value }, label));
}

export function shareOverlay(icon: Icon, name: string, links: ShareLink[], copied: string | null, now: number): Node[] {
  const cell = 'px-3 py-2 text-left font-normal';
  const rows = links.map((link) =>
    el(
      'tr',
      { class: 'border-t border-border-primary' },
      el('td', { class: `${cell} text-white` }, createdLabel(link.created, now)),
      el('td', { class: `${cell} text-text-secondary` }, accessLabels[link.access]),
      el('td', { class: `${cell} text-text-secondary` }, expiryLabels[link.expires]),
      el(
        'td',
        { class: 'px-1' },
        el(
          'span',
          { class: 'flex justify-end' },
          iconButton(icon('external-link', 'h-3.5 w-3.5'), 'Open as a guest', { 'data-action': 'share-open', 'data-token': link.token }),
          iconButton(icon(copied === link.token ? 'check' : 'copy', 'h-3.5 w-3.5'), 'Copy link', { 'data-action': 'share-copy', 'data-token': link.token }),
          iconButton(icon('trash', 'h-3.5 w-3.5'), 'Delete link', { 'data-action': 'share-revoke', 'data-token': link.token })
        )
      )
    )
  );
  const field = (label: string, control: HTMLElement) => el('label', { class: 'flex flex-1 flex-col gap-1.5 text-text-secondary' }, label, control);

  return [
    backdrop('bg-black/60'),
    el(
      'div',
      { role: 'dialog', 'aria-label': `Share ${name}`, class: `${panel} max-h-full w-full max-w-[440px]` },
      header(icon, `Share ${name}`, ''),
      el(
        'div',
        { class: 'mock-scrollbar flex flex-col gap-4 overflow-y-auto p-4' },
        links.length > 0 &&
          el(
            'div',
            { class: 'overflow-x-auto rounded border border-border-primary' },
            el(
              'table',
              { class: 'w-full text-[12px]' },
              el('thead', {}, el('tr', { class: 'text-text-secondary' }, el('th', { class: cell }, 'Created'), el('th', { class: cell }, 'Access'), el('th', { class: cell }, 'Expires'), el('th', {}))),
              el('tbody', {}, ...rows)
            )
          ),
        el(
          'div',
          { class: 'flex gap-3' },
          field('Access', el('select', { name: 'access', class: select }, ...options(accessLabels))),
          field('Expires', el('select', { name: 'expires', class: select }, ...options(expiryLabels)))
        )
      ),
      el(
        'div',
        { class: 'flex shrink-0 justify-end border-t border-border-secondary p-3' },
        el('button', { type: 'button', 'data-action': 'share-create', class: 'rounded bg-accent px-4 py-2 font-medium text-[#0d0d0d] transition-colors hover:bg-white' }, 'Create link')
      )
    )
  ];
}

export type MenuItem = { label: string; action: string; icon: IconName; disabled?: boolean };
export type MenuEntry = MenuItem | 'separator';

/** `x` and `y` are in the preview's own pixels, from its top-left corner. */
export function contextMenu(icon: Icon, x: number, y: number, items: MenuEntry[]): Node[] {
  return [
    backdrop(''),
    el(
      'div',
      { role: 'menu', style: `left: ${x}px; top: ${y}px`, class: 'demo-pop absolute w-44 overflow-hidden rounded-md border border-border-primary bg-surface-secondary py-1 shadow-2xl' },
      ...items.map((item) =>
        item === 'separator'
          ? el('div', { role: 'separator', class: 'my-1 border-t border-border-primary' })
          : el('button', { type: 'button', role: 'menuitem', 'data-action': item.action, disabled: item.disabled, class: 'flex w-full items-center gap-2.5 px-3 py-1.5 text-left enabled:hover:bg-selection disabled:text-text-disabled' }, icon(item.icon, 'h-3.5 w-3.5 text-text-secondary'), item.label)
      )
    )
  ];
}

export function themePicker(themes: readonly Theme[], active: Theme): Node[] {
  return [
    backdrop(''),
    el(
      'div',
      { role: 'dialog', 'aria-label': 'Theme', class: 'demo-pop absolute top-11 right-3 w-48 overflow-hidden rounded-md border border-border-primary bg-surface-secondary py-2 shadow-2xl' },
      el('div', { class: 'px-3 pb-1.5 text-[11px] font-medium tracking-wide text-text-secondary uppercase' }, 'Theme'),
      ...themes.map((theme, index) =>
        el(
          'button',
          { type: 'button', 'data-action': 'theme', 'data-index': String(index), 'aria-pressed': String(theme === active), class: 'flex w-full items-center gap-2.5 px-3 py-1.5 text-left hover:bg-surface-elevated aria-pressed:text-white' },
          el('span', { style: `background: ${theme.accent}`, class: 'h-3.5 w-3.5 rounded-full ring-white ring-offset-2 ring-offset-surface-secondary in-aria-pressed:ring-1' }),
          theme.name
        )
      )
    )
  ];
}
