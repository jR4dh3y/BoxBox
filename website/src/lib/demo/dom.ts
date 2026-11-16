// DOM building for the preview. Nodes are built, never parsed from strings, so
// a file name can hold anything.

export const iconNames = [
  'check',
  'chevron-left',
  'chevron-right',
  'copy',
  'download',
  'external-link',
  'file-plus',
  'file-text',
  'folder',
  'folder-plus',
  'folder-up',
  'house',
  'loader',
  'lock',
  'rotate-ccw',
  'share',
  'star',
  'trash',
  'upload',
  'x',
  'zoom-in',
  'zoom-out'
] as const;

export type IconName = (typeof iconNames)[number];
export type Icon = (name: IconName, className?: string) => Node;

export type Attrs = Record<string, string | boolean | undefined>;
export type Child = Node | string | false | null | undefined;

export function el<K extends keyof HTMLElementTagNameMap>(tag: K, attrs: Attrs = {}, ...children: Child[]): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  for (const [name, value] of Object.entries(attrs)) {
    if (value === true) node.setAttribute(name, '');
    else if (typeof value === 'string') node.setAttribute(name, value);
  }
  fill(node, ...children);
  return node;
}

/** Replaces what `node` holds, skipping the children a condition left out. */
export function fill(node: Element, ...children: Child[]): void {
  node.replaceChildren(...children.filter((child): child is Node | string => child !== false && child !== null && child !== undefined));
}

/** Icons are rendered once by DemoApp.astro into templates and cloned here. */
export function iconsFrom(root: ParentNode): Icon {
  return (name, className = 'h-4 w-4') => {
    const template = root.querySelector<HTMLTemplateElement>(`template[data-icon="${name}"]`);
    const svg = template?.content.firstElementChild?.cloneNode(true);
    if (!(svg instanceof SVGElement)) return document.createTextNode('');
    svg.setAttribute('class', className);
    svg.setAttribute('aria-hidden', 'true');
    return svg;
  };
}

export function part<T extends HTMLElement>(root: ParentNode, name: string, type: new () => T): T {
  const node = root.querySelector(`[data-part="${name}"]`);
  if (!(node instanceof type)) throw new Error(`The preview is missing its "${name}" part.`);
  return node;
}

export function iconButton(icon: Node, label: string, attrs: Attrs = {}): HTMLButtonElement {
  return el(
    'button',
    {
      type: 'button',
      'aria-label': label,
      title: label,
      class: 'flex h-7 w-7 shrink-0 items-center justify-center rounded text-text-secondary transition-colors hover:bg-surface-elevated hover:text-text-primary disabled:cursor-not-allowed disabled:text-text-disabled disabled:hover:bg-transparent',
      ...attrs
    },
    icon
  );
}
