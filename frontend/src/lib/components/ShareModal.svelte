<script lang="ts">
	/**
	 * ShareModal - Create and manage share links for a single file
	 */
	import { Check, Copy, Link2, Trash2 } from 'lucide-svelte';
	import { Badge, Button, Modal, Select, Spinner, Toggle } from '$lib/components/ui';
	import {
		createShare,
		hasShareExpiry,
		listShares,
		revokeShare,
		type CreateShareResponse,
		type FileInfo,
		type SharePermissions,
		type ShareRecord
	} from '$lib/api';
	import { toastStore } from '$lib/stores/toast.svelte';
	import { formatDate } from '$lib/utils/format';

	interface Props {
		open?: boolean;
		file?: FileInfo | null;
		onclose?: () => void;
	}

	let { open = false, file = null, onclose }: Props = $props();

	const EXPIRY_OPTIONS = [
		{ value: '3600', label: '1 hour' },
		{ value: '86400', label: '1 day' },
		{ value: '604800', label: '7 days' },
		{ value: '2592000', label: '30 days' },
		{ value: '0', label: 'Never' }
	];

	let shares = $state<ShareRecord[]>([]);
	let loading = $state(false);
	let creating = $state(false);
	let error = $state<string | null>(null);
	let permissions = $state<SharePermissions>({ view: true, download: true, write: false });
	let expiry = $state('0');
	let created = $state<CreateShareResponse | null>(null);
	let copied = $state(false);

	const activeShares = $derived.by(() => {
		if (!file) return [];
		return shares.filter((share) => share.path === file.path);
	});
	const createdUrl = $derived(created ? `${window.location.origin}${created.url}` : '');

	$effect(() => {
		if (open && file) {
			permissions = { view: true, download: true, write: false };
			expiry = '0';
			created = null;
			copied = false;
			error = null;
			void loadShares();
		}
	});

	async function loadShares() {
		loading = true;
		error = null;
		try {
			const response = await listShares();
			shares = response.shares;
		} catch (err) {
			error = err instanceof Error ? err.message : 'Unable to load share links.';
		} finally {
			loading = false;
		}
	}

	async function handleCreate() {
		if (!file || creating) return;
		creating = true;
		error = null;
		try {
			const expiresInSeconds = expiry === '0' ? undefined : Number(expiry);
			created = await createShare(file.path, {
				permissions,
				...(expiresInSeconds ? { expiresInSeconds } : {})
			});
			copied = false;
			await loadShares();
		} catch (err) {
			error = err instanceof Error ? err.message : 'Unable to create share link.';
		} finally {
			creating = false;
		}
	}

	async function handleRevoke(share: ShareRecord) {
		try {
			await revokeShare(share.id);
			toastStore.success(`Share link for ${share.fileName} revoked`);
			if (created && created.id === share.id) {
				created = null;
				copied = false;
			}
			await loadShares();
		} catch (err) {
			toastStore.error(err instanceof Error ? err.message : 'Unable to revoke share link.');
		}
	}

	async function handleCopy() {
		if (!createdUrl || copied) return;
		try {
			await navigator.clipboard.writeText(createdUrl);
			copied = true;
			toastStore.success('Share link copied to clipboard');
			setTimeout(() => (copied = false), 2000);
		} catch {
			toastStore.error('Unable to copy share link');
		}
	}

	function formatExpiry(share: ShareRecord): string {
		if (!hasShareExpiry(share.expiresAt)) return 'Never expires';
		return `Expires ${formatDate(share.expiresAt, { relative: true })}`;
	}
</script>

<Modal {open} title="Share File" {onclose}>
	{#if file}
		<p class="mt-0 mb-4 truncate text-sm text-text-secondary" title={file.path}>{file.path}</p>

		{#if error}
			<div
				class="mb-4 rounded border border-danger/30 bg-danger/20 px-3 py-2 text-sm text-danger"
				role="alert"
			>
				{error}
			</div>
		{/if}

		<section class="mb-4">
			<h3 class="mb-2 text-sm font-medium text-text-primary">Active share links</h3>
			{#if loading}
				<div class="flex justify-center py-4"><Spinner /></div>
			{:else if activeShares.length === 0}
				<p class="m-0 text-sm text-text-secondary">No active share links for this file.</p>
			{:else}
				<ul class="m-0 flex list-none flex-col gap-2 p-0">
					{#each activeShares as share (share.id)}
						<li
							class="flex flex-wrap items-center gap-2 rounded border border-border-primary bg-surface-tertiary px-3 py-2"
						>
							<Link2 size={16} class="shrink-0 text-accent" />
							<span class="min-w-0 flex-1 truncate text-sm text-text-primary" title={share.url}>
								{share.url}
							</span>
							<span class="text-xs text-text-muted">{formatExpiry(share)}</span>
							<span class="flex items-center gap-1">
								{#if share.permissions.view}<Badge variant="info">View</Badge>{/if}
								{#if share.permissions.download}<Badge variant="success">Download</Badge>{/if}
								{#if share.permissions.write}<Badge variant="warning">Write</Badge>{/if}
							</span>
							<Button
								variant="ghost"
								size="icon"
								title="Revoke share link"
								onclick={() => void handleRevoke(share)}
							>
								<Trash2 size={16} />
							</Button>
						</li>
					{/each}
				</ul>
			{/if}
		</section>

		{#if created}
			<section class="mb-4">
				<h3 class="mb-2 text-sm font-medium text-text-primary">New share link</h3>
				<div class="flex items-center gap-2">
					<input
						type="text"
						readonly
						value={createdUrl}
						class="h-8 min-w-0 flex-1 rounded border border-border-primary bg-surface-primary px-3 text-sm text-text-primary"
						aria-label="Share link URL"
					/>
					<Button variant="secondary" size="sm" onclick={() => void handleCopy()}>
						{#if copied}
							<Check size={16} />
							<span>Copied</span>
						{:else}
							<Copy size={16} />
							<span>Copy</span>
						{/if}
					</Button>
				</div>
			</section>
		{/if}

		<section class="flex flex-col gap-3">
			<h3 class="mb-0 text-sm font-medium text-text-primary">Create a share link</h3>
			<div class="flex flex-wrap items-center gap-4">
				<Toggle id="share-permission-view" label="View" bind:checked={permissions.view} />
				<Toggle
					id="share-permission-download"
					label="Download"
					bind:checked={permissions.download}
				/>
				<Toggle id="share-permission-write" label="Write" bind:checked={permissions.write} />
			</div>
			{#if permissions.write}
				<p class="m-0 text-xs text-text-muted">
					Write lets recipients replace this file through the share link.
				</p>
			{/if}
			<div class="flex flex-col gap-2">
				<label for="share-expiry" class="text-sm font-medium text-text-secondary">Expires</label>
				<Select id="share-expiry" options={EXPIRY_OPTIONS} bind:value={expiry} />
			</div>
			<Button
				variant="primary"
				disabled={creating ||
					loading ||
					(!permissions.view && !permissions.download && !permissions.write)}
				onclick={() => void handleCreate()}
			>
				{#if creating}
					<Spinner size="sm" />
					<span>Creating...</span>
				{:else}
					<span>Create Share Link</span>
				{/if}
			</Button>
		</section>
	{/if}
</Modal>
