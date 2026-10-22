export type DesktopPanel = 'trash' | 'volumes';

/** The desktop panel open over the browser; the sidebar opens it, DesktopPanels renders it. */
export const desktopPanel = $state<{ current: DesktopPanel | null }>({ current: null });
