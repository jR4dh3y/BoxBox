<script lang="ts">
	import { Check, Copy, FolderOpen, Link2, Trash2 } from 'lucide-svelte';
	import { Button, Modal, Select, Spinner } from '$lib/components/ui';
	import {
		createShare,
		hasShareExpiry,
		listShares,
		revokeShare,
		type CreateShareResponse,
		type FileInfo,
		type SharePermissionInput,
		type ShareRecord
	} from '$lib/api';
	import { toastStore } from '$lib/stores/toast.svelte';
	import { formatDate, formatFileSize } from '$lib/utils/format';
	import { getShareAccessLabel } from '$lib/utils/shareAccess';

	interface Props {
		open?: boolean;
		folder?: FileInfo | null;
		onclose?: () => void;
	}

	let { open = false, folder = null, onclose }: Props = $props();
	const EXPIRY_OPTIONS = [
		{ value: '0', label: 'Never' },
		{ value: '604800', label: '1 week' },
		{ value: '2592000', label: '30 days' }
	];
	type FolderShareAccess = 'view' | 'upload' | 'upload-delete';
	const ACCESS_OPTIONS: Array<{ value: FolderShareAccess; label: string; detail: string }> = [
		{ value: 'view', label: 'View only', detail: 'Browse and download files' },
		{ value: 'upload', label: 'Upload only', detail: 'Add files, but not replace or delete' },
		{ value: 'upload-delete', label: 'Upload + delete', detail: 'Add, replace, and remove files' }
	];
	let access = $state<FolderShareAccess>('view');
	let maxUploadMB = $state<number | undefined>(undefined);
	let expiry = $state('0');
	let creating = $state(false);
	let loading = $state(false);
	let error = $state<string | null>(null);
	let created = $state<CreateShareResponse | null>(null);
	let copied = $state(false);
	let shares = $state<ShareRecord[]>([]);
	const createdUrl = $derived(created ? `${window.location.origin}${created.url}` : '');
	const activeShares = $derived.by(() => {
		if (!folder) return [];
		return shares.filter((share) => share.isFolder && share.path === folder.path);
	});

	$effect(() => {
		if (open && folder) {
			access = 'view';
			maxUploadMB = undefined;
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
			error = err instanceof Error ? err.message : 'Unable to load folder share links.';
		} finally {
			loading = false;
		}
	}

	async function handleCreate() {
		if (!folder || creating) return;
		if (
			access !== 'view' &&
			maxUploadMB !== undefined &&
			(!Number.isSafeInteger(maxUploadMB) || maxUploadMB < 1)
		) {
			error = 'Enter a positive whole number for the upload limit.';
			return;
		}
		creating = true;
		error = null;
		try {
			const permissions: SharePermissionInput = {
				view: true,
				download: true,
				upload: access !== 'view',
				delete: access === 'upload-delete'
			};
			created = await createShare(folder.path, {
				permissions,
				...(access !== 'view' && maxUploadMB !== undefined
					? { maxUploadBytes: maxUploadMB * 1024 * 1024 }
					: {}),
				...(expiry !== '0' ? { expiresInSeconds: Number(expiry) } : {})
			});
			await loadShares();
		} catch (err) {
			error = err instanceof Error ? err.message : 'Unable to create folder share.';
		} finally {
			creating = false;
		}
	}

	async function handleRevoke(share: ShareRecord) {
		try {
			await revokeShare(share.id);
			toastStore.success(`Folder share for ${share.fileName} revoked`);
			if (created?.id === share.id) {
				created = null;
				copied = false;
			}
			await loadShares();
		} catch (err) {
			toastStore.error(err instanceof Error ? err.message : 'Unable to revoke folder share.');
		}
	}

	async function copyLink() {
		if (!createdUrl) return;
		try {
			await navigator.clipboard.writeText(createdUrl);
			copied = true;
			toastStore.success('Folder link copied to clipboard');
		} catch {
			toastStore.error('Unable to copy folder link');
		}
	}

	function formatExpiry(share: ShareRecord): string {
		if (!hasShareExpiry(share.expiresAt)) return 'Never expires';
		return `Expires ${formatDate(share.expiresAt, { relative: true })}`;
	}
</script>

<Modal {open} title="Share folder" {onclose}>
	{#if folder}
		<div class="mb-5 flex items-center gap-3">
			<FolderOpen size={24} class="shrink-0 text-folder" />
			<div class="min-w-0">
				<h3 class="m-0 truncate text-sm font-medium text-text-primary">{folder.name}</h3>
				<p class="m-0 truncate text-xs text-text-muted" title={folder.path}>{folder.path}</p>
			</div>
		</div>
		{#if error}<p
				class="mb-4 rounded border border-danger/30 bg-danger/20 px-3 py-2 text-sm text-danger"
				role="alert"
			>
				{error}
			</p>{/if}
		{#if created}
			<div class="mb-5 rounded border border-accent/30 bg-accent/10 p-3">
				<p class="m-0 mb-2 text-sm font-medium text-text-primary">Folder link ready</p>
				<div class="flex gap-2">
					<input
						readonly
						value={createdUrl}
						aria-label="Folder share link"
						class="h-8 min-w-0 flex-1 rounded border border-border-primary bg-surface-primary px-2 text-xs text-text-primary"
					/>
					<Button variant="secondary" size="sm" onclick={() => void copyLink()}
						>{#if copied}<Check size={15} />Copied{:else}<Copy size={15} />Copy{/if}</Button
					>
				</div>
			</div>
		{/if}
		<section class="mb-5">
			<h3 class="mb-2 text-sm font-medium text-text-primary">Active folder links</h3>
			{#if loading}
				<div class="flex justify-center py-4"><Spinner /></div>
			{:else if activeShares.length === 0}
				<p class="m-0 text-sm text-text-secondary">No active folder links for this folder.</p>
			{:else}
				<ul class="m-0 flex list-none flex-col gap-2 p-0">
					{#each activeShares as share (share.id)}
						<li
							class="flex flex-wrap items-center gap-2 rounded border border-border-primary bg-surface-tertiary px-3 py-2"
						>
							<Link2 size={16} class="shrink-0 text-accent" />
							<span class="min-w-0 flex-1 truncate text-sm text-text-primary" title={share.url}
								>{share.url}</span
							>
							<span class="text-xs text-text-muted">{getShareAccessLabel(share.permissions)}</span>
							{#if share.permissions.upload}<span class="text-xs text-text-muted"
									>Max {formatFileSize(share.maxUploadBytes)} per file</span
								>{/if}
							<span class="text-xs text-text-muted">{formatExpiry(share)}</span>
							<Button
								variant="ghost"
								size="icon"
								title="Revoke folder share"
								onclick={() => void handleRevoke(share)}
							>
								<Trash2 size={16} />
							</Button>
						</li>
					{/each}
				</ul>
			{/if}
		</section>
		<div class="flex flex-col gap-4">
			<div>
				<p class="mb-2 text-sm font-medium text-text-primary">Folder access</p>
				<div class="grid gap-2 sm:grid-cols-3">
					{#each ACCESS_OPTIONS as option (option.value)}
						<button
							type="button"
							class="rounded border px-3 py-2 text-left {access === option.value
								? 'border-accent bg-accent/10'
								: 'border-border-primary'}"
							aria-pressed={access === option.value}
							onclick={() => (access = option.value)}
						>
							<span class="block text-sm font-medium text-text-primary">{option.label}</span>
							<span class="block text-xs text-text-muted">{option.detail}</span>
						</button>
					{/each}
				</div>
			</div>
			{#if access !== 'view'}
				<div>
					<label
						for="folder-share-upload-limit"
						class="mb-2 block text-sm font-medium text-text-secondary"
						>Maximum upload size per file (MB)</label
					>
					<input
						id="folder-share-upload-limit"
						type="number"
						min="1"
						step="1"
						bind:value={maxUploadMB}
						placeholder="Server maximum"
						class="h-9 w-full rounded border border-border-primary bg-surface-primary px-3 text-sm text-text-primary placeholder:text-text-muted"
					/>
					<p class="mt-1 text-xs text-text-muted">Leave blank to use the server's maximum.</p>
				</div>
			{/if}
			<div>
				<label for="folder-share-expiry" class="mb-2 block text-sm font-medium text-text-secondary"
					>Link expires</label
				><Select id="folder-share-expiry" options={EXPIRY_OPTIONS} bind:value={expiry} />
			</div>
			<Button variant="primary" disabled={creating} onclick={() => void handleCreate()}
				>{#if creating}<Spinner size="sm" />Creating link{:else}Create folder link{/if}</Button
			>
		</div>
	{/if}
</Modal>
