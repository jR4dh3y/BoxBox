export async function sha256Hex(blob: Blob): Promise<string | null> {
	if (!globalThis.crypto?.subtle) return null;
	const digest = await crypto.subtle.digest('SHA-256', await blob.arrayBuffer());
	return Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, '0')).join('');
}
