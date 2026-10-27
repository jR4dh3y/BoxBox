import type { UploadProgress } from '#lib/utils/upload.js';

/** Announces each terminal success once, including completions in the same update. */
export function createUploadAnnouncements(
	uploads: () => readonly Pick<UploadProgress, 'uploadId' | 'fileName' | 'status'>[]
) {
	let message = $state('');
	// This history is not rendered; only the message is reactive.
	// eslint-disable-next-line svelte/prefer-svelte-reactivity
	const announced = new Set<string>();
	$effect(() => {
		const completed: string[] = [];
		for (const upload of uploads()) {
			if (upload.status !== 'complete' || announced.has(upload.uploadId)) continue;
			announced.add(upload.uploadId);
			completed.push(upload.fileName);
		}
		if (completed.length > 0) {
			message = `${completed.join(', ')} uploaded. ${announced.size} complete.`;
		}
	});
	return {
		get message() {
			return message;
		}
	};
}
