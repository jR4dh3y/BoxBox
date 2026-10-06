/**
 * Copy text to the clipboard. Browsers only expose navigator.clipboard on secure
 * origins (HTTPS or localhost), so plain-HTTP access over a LAN IP falls back to
 * a hidden textarea and execCommand('copy').
 */
export async function copyText(text: string): Promise<void> {
	if (window.isSecureContext && navigator.clipboard) {
		await navigator.clipboard.writeText(text);
		return;
	}

	const textarea = document.createElement('textarea');
	textarea.value = text;
	textarea.setAttribute('readonly', '');
	textarea.style.position = 'fixed';
	textarea.style.opacity = '0';
	document.body.appendChild(textarea);
	textarea.select();
	try {
		if (!document.execCommand('copy')) throw new Error('Copy was blocked by the browser.');
	} finally {
		textarea.remove();
	}
}
