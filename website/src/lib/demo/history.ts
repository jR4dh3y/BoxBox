// Back and forward for the preview's location, like a browser tab.

export type History = { entries: string[]; index: number };

export function currentPath(history: History): string {
  return history.entries[history.index] ?? '';
}

export function visit(history: History, path: string): History {
  if (currentPath(history) === path) return history;
  const entries = [...history.entries.slice(0, history.index + 1), path];
  return { entries, index: entries.length - 1 };
}

export function canGoBack(history: History): boolean {
  return history.index > 0;
}

export function canGoForward(history: History): boolean {
  return history.index < history.entries.length - 1;
}

export function back(history: History): History {
  return canGoBack(history) ? { ...history, index: history.index - 1 } : history;
}

export function forward(history: History): History {
  return canGoForward(history) ? { ...history, index: history.index + 1 } : history;
}
