export function isLocalDesktop(): boolean {
	return (
		typeof document !== 'undefined' &&
		document.querySelector('meta[name="boxbox-desktop"]') !== null
	);
}

export async function openDesktopFile(path: string): Promise<void> {
	const response = await fetch(`/api/v1/desktop/open?${new URLSearchParams({ path })}`, {
		method: 'POST'
	});
	if (!response.ok) throw new Error('Could not open this file in its default application');
}

export function openConnectionManager(): void {
	if ('__electrobunSendToHost' in window && typeof window.__electrobunSendToHost === 'function') {
		window.__electrobunSendToHost({ type: 'boxbox:connections' });
	}
}
