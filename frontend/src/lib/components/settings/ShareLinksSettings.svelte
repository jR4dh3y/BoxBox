<script lang="ts">
	import { onMount } from 'svelte';
	import { Copy, RefreshCw, Trash2 } from 'lucide-svelte';
	import { Button, Spinner } from '$lib/components/ui';
	import { hasShareExpiry, listShares, revokeShare, type ShareRecord } from '$lib/api';
	import { getShareAccessLabel } from '$lib/utils/shareAccess';
	import { formatDate, formatFileSize } from '$lib/utils/format';

	let shares = $state<ShareRecord[]>([]);
	let loading = $state(true);
	let revokingId = $state<string | null>(null);
	let error = $state<string | null>(null);
	let message = $state<string | null>(null);

	onMount(() => {
		void loadShares();
	});

	async function loadShares() {
		loading = true;
		error = null;
		message = null;
		try {
			shares = (await listShares()).shares;
		} catch (err) {
			error = err instanceof Error ? err.message : 'Unable to load share links.';
		} finally {
			loading = false;
		}
	}

	async function handleRevoke(share: ShareRecord) {
		if (revokingId) return;
		revokingId = share.id;
		error = null;
		message = null;
		try {
			await revokeShare(share.id);
			shares = shares.filter((item) => item.id !== share.id);
			message = `Share link for ${share.fileName} revoked.`;
		} catch (err) {
			error = err instanceof Error ? err.message : 'Unable to revoke this share link.';
		} finally {
			revokingId = null;
		}
	}

	async function handleCopy(share: ShareRecord) {
		try {
			await navigator.clipboard.writeText(new URL(share.url, window.location.origin).href);
			message = 'Share link copied.';
			error = null;
		} catch {
			error = 'Unable to copy this share link.';
			message = null;
		}
	}

	function formatExpiry(share: ShareRecord): string {
		if (!hasShareExpiry(share.expiresAt)) return 'Never expires';
		return `Expires ${formatDate(share.expiresAt, { relative: true })}`;
	}
</script>

<div class="p-4">
	<div class="mb-4 flex flex-wrap items-center justify-between gap-3">
		<p class="m-0 text-sm text-text-secondary">Review and revoke active file and folder links.</p>
		<Button
			variant="secondary"
			size="sm"
			disabled={loading || revokingId !== null}
			onclick={() => void loadShares()}
		>
			<RefreshCw size={15} />Refresh
		</Button>
	</div>
	{#if error}<p class="mb-3 text-sm text-danger" role="alert">{error}</p>{/if}
	{#if message}<p class="mb-3 text-sm text-success" role="status">{message}</p>{/if}
	{#if loading}
		<div class="flex justify-center py-8"><Spinner /></div>
	{:else if shares.length === 0}
		<p
			class="m-0 rounded border border-border-primary bg-surface-primary px-4 py-6 text-center text-sm text-text-secondary"
		>
			No active share links.
		</p>
	{:else}
		<ul class="m-0 flex list-none flex-col gap-3 p-0">
			{#each shares as share (share.id)}
				<li
					class="flex flex-wrap items-center gap-3 rounded border border-border-primary bg-surface-primary px-3 py-3"
				>
					<div class="min-w-0 flex-1">
						<a
							href={share.url}
							target="_blank"
							rel="noopener noreferrer"
							class="block truncate text-sm font-medium text-text-primary hover:text-accent"
							title={share.url}>{share.fileName}</a
						>
						<p class="m-0 mt-1 truncate text-xs text-text-muted" title={share.path}>{share.path}</p>
						<p class="m-0 mt-1 text-xs text-text-secondary">
							{share.isFolder ? 'Folder' : 'File'} · {getShareAccessLabel(share.permissions)} · {formatExpiry(
								share
							)}
							{#if share.permissions.upload}
								· Max {formatFileSize(share.maxUploadBytes)} per file{/if}
						</p>
					</div>
					<div class="flex shrink-0 gap-2">
						<Button
							variant="secondary"
							size="sm"
							title="Copy share link"
							onclick={() => void handleCopy(share)}
						>
							<Copy size={15} />Copy
						</Button>
						<Button
							variant="danger"
							size="sm"
							disabled={revokingId !== null}
							title="Revoke share link"
							onclick={() => void handleRevoke(share)}
						>
							{#if revokingId === share.id}<Spinner size="sm" />{:else}<Trash2
									size={15}
								/>{/if}Revoke
						</Button>
					</div>
				</li>
			{/each}
		</ul>
	{/if}
</div>
