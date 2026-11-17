// The landing page borrows the app's themes: once the interface preview is on
// screen the accent colour of the whole page follows it, cycling until the
// visitor picks a theme themselves.

export type Theme = { name: string; wallpaper: string; accent: string; glow: string };

export const themes: readonly Theme[] = [
  { name: 'Evernight', wallpaper: '/wallpapers/evernight.png', accent: '#a892ff', glow: 'rgba(146, 124, 255, 0.26)' },
  { name: 'Daylight', wallpaper: '/wallpapers/light.png', accent: '#5ea8ff', glow: 'rgba(94, 168, 255, 0.28)' },
  { name: 'Lucy', wallpaper: '/wallpapers/lucy.jpeg', accent: '#ff7ab8', glow: 'rgba(255, 122, 184, 0.28)' },
  { name: 'Minami', wallpaper: '/wallpapers/minami.jpg', accent: '#7ed3ff', glow: 'rgba(126, 211, 255, 0.24)' },
  { name: 'Summer', wallpaper: '/wallpapers/summer_1.png', accent: '#ff9f70', glow: 'rgba(255, 159, 112, 0.26)' }
];

const CYCLE_MS = 4800;

function channels(hex: string): [number, number, number] {
  const value = Number.parseInt(hex.replace('#', ''), 16);
  return [(value >> 16) & 255, (value >> 8) & 255, value & 255];
}

export function mixHex(color: string, target: string, amount: number): string {
  const from = channels(color);
  const to = channels(target);
  const mixed = from.map((channel, index) => Math.round(channel + ((to[index] ?? channel) - channel) * amount));
  return `#${mixed.map((channel) => channel.toString(16).padStart(2, '0')).join('')}`;
}

/** The colour variables every accent-coloured thing on the page reads. */
export function accentVars(accent: string): [string, string][] {
  return [
    ['--color-accent', accent],
    ['--color-border-focus', accent],
    ['--color-accent-hover', mixHex(accent, '#000000', 0.28)],
    ['--color-accent-muted', mixHex(accent, '#1b1b1b', 0.58)],
    ['--color-selection', mixHex(accent, '#1e1e1e', 0.6)],
    ['--color-selection-hover', mixHex(accent, '#000000', 0.35)]
  ];
}

export function applyAccent(target: HTMLElement, theme: Theme): void {
  for (const [name, value] of accentVars(theme.accent)) target.style.setProperty(name, value);
}

type Listener = (theme: Theme) => void;

const listeners = new Set<Listener>();
let active: Theme = themes[0];
let started = false;
let pinned = false;
let timer: number | undefined;

export function activeTheme(): Theme {
  return active;
}

export function onThemeChange(listener: Listener): void {
  listeners.add(listener);
}

function show(theme: Theme): void {
  active = theme;
  const root = document.documentElement;
  root.dataset.accentTransition = 'true';
  applyAccent(root, theme);
  for (const listener of listeners) listener(theme);
}

function syncTimer(): void {
  window.clearInterval(timer);
  timer = undefined;
  if (!started || pinned || window.matchMedia('(prefers-reduced-motion: reduce)').matches) return;
  timer = window.setInterval(() => show(themes[(themes.indexOf(active) + 1) % themes.length]), CYCLE_MS);
}

/** Starts colouring the page with the themes. Safe to call more than once. */
export function startThemeCycle(): void {
  if (started) return;
  started = true;
  show(active);
  syncTimer();
  window.matchMedia('(prefers-reduced-motion: reduce)').addEventListener('change', syncTimer);
}

/** The visitor chose a theme: show it and stop cycling. */
export function pinTheme(theme: Theme): void {
  started = true;
  pinned = true;
  syncTimer();
  show(theme);
}
