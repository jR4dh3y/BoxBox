// Share links in the preview. The option values come back from <select>
// elements as strings, so they are parsed here before anything trusts them.

export type Access = 'view' | 'download';
export type Expiry = 'never' | 'hour' | 'day' | 'week';
export type ShareLink = { token: string; path: string; access: Access; expires: Expiry; created: number };

export const accessLabels: Record<Access, string> = {
  view: 'View only',
  download: 'View and download'
};

export const expiryLabels: Record<Expiry, string> = {
  never: 'Never',
  hour: 'in 1 hour',
  day: 'in 1 day',
  week: 'in 1 week'
};

export function parseAccess(value: string): Access {
  return value === 'download' ? 'download' : 'view';
}

export function parseExpiry(value: string): Expiry {
  return value === 'hour' || value === 'day' || value === 'week' ? value : 'never';
}

export function expirySentence(expires: Expiry): string {
  return expires === 'never' ? 'Never expires' : `Expires ${expiryLabels[expires]}`;
}

export function createdLabel(created: number, now: number): string {
  const minutes = Math.floor((now - created) / 60_000);
  if (minutes < 1) return 'just now';
  return minutes === 1 ? '1 minute ago' : `${minutes} minutes ago`;
}

export function newToken(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(6));
  return Array.from(bytes, (byte) => byte.toString(36).padStart(2, '0')).join('').slice(0, 10);
}
