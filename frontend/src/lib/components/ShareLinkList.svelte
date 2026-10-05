<script lang="ts">
	/**
	 * ShareLinkList - table of share links with copy and revoke, shared by the share dialog and settings.
	 */
	import { Check, Copy, Trash2 } from 'lucide-svelte';
	import { hasShareExpiry, revokeShare, type ShareRecord } from '$lib/api';
	import { copyText } from '$lib/utils/clipboard';
	import { getFileIcon } from '$lib/utils/fileTypes';
	import { formatFileSize, formatRelativeTime } from '$lib/utils/format';
	import { getShareAccessLabel } from '$lib/utils/shareAccess';

	interface Props {
		shares: ShareRecord[];
		/** Show the shared item's path instead of when the link was created (used when listing every share). */
		showItem?: boolean;
		showAccess?: boolean;
		highlightId?: string | null;
		onrevoked?: (share: ShareRecord) => void;
	}

	let {
		shares,
		showItem = false,
		showAccess = true,
		highlightId = null,
		onrevoked
	}: Props = $props();

	let copiedId = $state<string | null>(null);
	let revokingId = $state<string | null>(null);
	let error = $state<string | null>(null);

	const thClass =
		'border-b border-border-primary bg-surface-secondary px-3 py-2 text-left font-medium whitespace-nowrap text-text-secondary';
	const tdClass = 'h-8 border-b border-border-secondary px-3 py-1.5 align-middle';
	const iconButtonClass =
		'flex h-7 w-7 cursor-pointer items-center justify-center rounded border-none bg-transparent text-text-secondary transition-colors duration-100 disabled:cursor-not-allowed disabled:opacity-50';

	async function handleCopy(share: ShareRecord) {
		try {
			await copyText(new URL(share.url, window.location.origin).href);
			error = null;
			copiedId = share.id;
			setTimeout(() => {
				if (copiedId === share.id) copiedId = null;
			}, 1500);
		} catch {
			error = 'Unable to copy this link.';
		}
	}

	async function handleRevoke(share: ShareRecord) {
		if (revokingId) return;
		revokingId = share.id;
		error = null;
		try {
			await revokeShare(share.id);
			onrevoked?.(share);
		} catch (err) {
			error = err instanceof Error ? err.message : 'Unable to revoke this link.';
		} finally {
			revokingId = null;
		}
	}

	function formatExpiry(share: ShareRecord): string {
		return hasShareExpiry(share.expiresAt) ? formatRelativeTime(share.expiresAt) : 'Never';
	}
</script>

{#if error}
	<p class="m-0 border-b border-border-secondary px-3 py-2 text-[13px] text-danger" role="alert">
		{error}
	</p>
{/if}
<!-- The first column takes the leftover width; the others size to their content and never wrap. -->
<table
	class="w-full border-collapse text-[13px] leading-5 whitespace-nowrap [&_tbody_tr:last-child_td]:border-b-0"
>
	<thead>
		<tr>
			<th class={thClass}>{showItem ? 'Item' : 'Created'}</th>
			{#if showAccess}<th class={thClass}>Access</th>{/if}
			<th class={thClass}>Expires</th>
			<th class={thClass}><span class="sr-only">Actions</span></th>
		</tr>
	</thead>
	<tbody>
		{#each shares as share (share.id)}
			{@const ItemIcon = getFileIcon(share.fileName, share.isFolder)}
			<tr class={share.id === highlightId ? 'bg-selection' : 'hover:bg-surface-secondary'}>
				<td class="{tdClass} w-full max-w-0">
					{#if showItem}
						<div class="flex min-w-0 items-center gap-2">
							<span
								class="flex w-5 shrink-0 justify-center {share.isFolder
									? 'text-folder'
									: 'text-text-secondary'}"><ItemIcon size={16} /></span
							>
							<span class="truncate text-text-primary" title={share.path}>{share.path}</span>
						</div>
					{:else}
						<span class="block truncate text-text-primary"
							>{formatRelativeTime(share.createdAt)}</span
						>
					{/if}
				</td>
				{#if showAccess}
					<td
						class="{tdClass} text-text-secondary"
						title={share.permissions.upload
							? `Max ${formatFileSize(share.maxUploadBytes)} per file`
							: undefined}>{getShareAccessLabel(share.permissions)}</td
					>
				{/if}
				<td class="{tdClass} text-text-secondary">{formatExpiry(share)}</td>
				<td class="{tdClass} px-1.5">
					<div class="flex justify-end gap-0.5">
						<button
							type="button"
							class="{iconButtonClass} hover:bg-surface-elevated hover:text-text-primary"
							title="Copy link"
							aria-label="Copy link"
							onclick={() => void handleCopy(share)}
						>
							{#if copiedId === share.id}<Check size={15} />{:else}<Copy size={15} />{/if}
						</button>
						<button
							type="button"
							class="{iconButtonClass} hover:bg-danger/15 hover:text-danger"
							title="Revoke link"
							aria-label="Revoke link"
							disabled={revokingId !== null}
							onclick={() => void handleRevoke(share)}
						>
							<Trash2 size={15} />
						</button>
					</div>
				</td>
			</tr>
		{/each}
	</tbody>
</table>
