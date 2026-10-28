import { isLocalDesktop } from '#lib/api/desktop.js';

/** Desktop locations include this computer and paired BoxBox deployments. */
export function rootLabel(): string {
	return isLocalDesktop() ? 'This PC' : 'This Server';
}
