<script lang="ts">
	/**
	 * ShareModal - list an item's share links and create new ones. Folders also choose recipient access;
	 * expiring file links on writable mounts can delete the file when they expire.
	 */
	import { Button, Modal, Select, Spinner } from '#lib/components/ui/index.js';
	import ShareLinkList from '#lib/components/ShareLinkList.svelte';
	import { type FileInfo, type SharePermissionInput, type ShareRecord } from '#lib/api/index.js';
	import { createShare, listShares } from '#lib/api/shareManagement.js';
	import { offersDeleteOnExpiry } from '#lib/utils/shareAccess.js';

	interface Props {
		open?: boolean;
		item?: FileInfo | null;
		/** The item's mount is read-only, so its file cannot be deleted on expiry. */
		readOnly: boolean;
		onclose?: () => void;
	}

	let { open = false, item = null, readOnly, onclose }: Props = $props();

	type FolderAccess = 'view' | 'upload' | 'upload-delete' | 'full';

	const ACCESS_OPTIONS: Array<{ value: FolderAccess; label: string }> = [
		{ value: 'view', label: 'View only' },
		{ value: 'upload', label: 'Upload only' },
		{ value: 'upload-delete', label: 'Upload + delete' },
		{ value: 'full', label: 'Full access' }
	];
	const EXPIRY_OPTIONS = [
		{ value: '0', label: 'Never' },
		{ value: '3600', label: '1 hour' },
		{ value: '86400', label: '1 day' },
		{ value: '604800', label: '7 days' },
		{ value: '2592000', label: '30 days' }
	];
	const labelClass = 'mb-1.5 block text-[13px] text-text-secondary';

	let shares = $state<ShareRecord[]>([]);
	let loading = $state(false);
	let creating = $state(false);
	let error = $state<string | null>(null);
	let access = $state<FolderAccess>('view');
	let expiry = $state('0');
	let maxUploadMB = $state<number | undefined>(undefined);
	let deleteOnExpiry = $state(false);
	let createdId = $state<string | null>(null);

	const itemShares = $derived(item ? shares.filter((share) => share.path === item.path) : []);
	const allowsUpload = $derived(item?.isDir === true && access !== 'view');
	const expiresInSeconds = $derived(expiry === '0' ? undefined : Number(expiry));
	const canDeleteOnExpiry = $derived(
		item !== null && offersDeleteOnExpiry({ isDir: item.isDir, readOnly, expiresInSeconds })
	);

	$effect(() => {
		if (open && item) {
			access = 'view';
			expiry = '0';
			maxUploadMB = undefined;
			deleteOnExpiry = false;
			createdId = null;
			error = null;
			void loadShares();
		}
	});

	async function loadShares() {
		loading = true;
		try {
			shares = (await listShares()).shares;
		} catch (err) {
			error = err instanceof Error ? err.message : 'Unable to load share links.';
		} finally {
			loading = false;
		}
	}

	async function handleCreate() {
		if (!item || creating) return;
		if (allowsUpload && maxUploadMB !== undefined) {
			if (!Number.isSafeInteger(maxUploadMB) || maxUploadMB < 1) {
				error = 'Max upload must be a whole number of MB.';
				return;
			}
		}
		creating = true;
		error = null;
		try {
			const permissions: SharePermissionInput | undefined = item.isDir
				? {
						view: true,
						download: true,
						upload: access !== 'view',
						delete: access === 'upload-delete' || access === 'full',
						manage: access === 'full'
					}
				: undefined;
			const created = await createShare(item.path, {
				...(permissions ? { permissions } : {}),
				...(allowsUpload && maxUploadMB !== undefined
					? { maxUploadBytes: maxUploadMB * 1024 * 1024 }
					: {}),
				...(expiresInSeconds !== undefined ? { expiresInSeconds } : {}),
				...(canDeleteOnExpiry && deleteOnExpiry ? { deleteOnExpiry: true } : {})
			});
			createdId = created.id;
			// Show the new link even if refreshing the list fails.
			shares = [{ ...created, path: item.path }, ...shares];
			await loadShares();
		} catch (err) {
			error = err instanceof Error ? err.message : 'Unable to create share link.';
		} finally {
			creating = false;
		}
	}

	function handleRevoked(share: ShareRecord) {
		shares = shares.filter((existing) => existing.id !== share.id);
	}
</script>

<Modal {open} title={item ? `Share ${item.name}` : 'Share'} {onclose}>
	{#if item}
		{#if error}
			<p class="m-0 mb-3 text-[13px] text-danger" role="alert">{error}</p>
		{/if}

		{#if loading && shares.length === 0}
			<div class="mb-4 flex justify-center py-3"><Spinner /></div>
		{:else if itemShares.length > 0}
			<div class="mb-4 overflow-hidden rounded border border-border-primary">
				<ShareLinkList
					shares={itemShares}
					showAccess={item.isDir}
					highlightId={createdId}
					onrevoked={handleRevoked}
				/>
			</div>
		{/if}

		<div class="grid gap-3 {item.isDir ? 'grid-cols-2' : ''}">
			{#if item.isDir}
				<div>
					<label for="share-access" class={labelClass}>Access</label>
					<Select id="share-access" options={ACCESS_OPTIONS} bind:value={access} />
				</div>
			{/if}
			<div>
				<label for="share-expiry" class={labelClass}>Expires</label>
				<!-- Switching to Never clears the choice, so a new expiry needs fresh intent. -->
				<Select
					id="share-expiry"
					options={EXPIRY_OPTIONS}
					bind:value={expiry}
					onchange={(value) => {
						if (value === '0') deleteOnExpiry = false;
					}}
				/>
			</div>
		</div>

		{#if canDeleteOnExpiry}
			<label class="mt-3 flex items-start gap-2 text-[13px] text-text-primary">
				<input type="checkbox" class="mt-0.5 accent-accent" bind:checked={deleteOnExpiry} />
				<span>
					Delete file when share expires
					<span class="block text-text-muted">
						The file is permanently deleted unless it changed or another active link shares it.
					</span>
				</span>
			</label>
		{/if}

		{#if allowsUpload}
			<div class="mt-3">
				<label for="share-upload-limit" class={labelClass}>Max upload per file (MB)</label>
				<input
					id="share-upload-limit"
					type="number"
					min="1"
					step="1"
					bind:value={maxUploadMB}
					placeholder="Server limit"
					class="h-8 w-full rounded border border-border-primary bg-surface-secondary px-3 text-sm text-text-primary placeholder:text-text-muted focus:border-border-focus focus:outline-none"
				/>
			</div>
		{/if}
	{/if}

	{#snippet footer()}
		<Button variant="primary" disabled={!item || creating} onclick={() => void handleCreate()}>
			{#if creating}<Spinner size="sm" />{/if}
			Create link
		</Button>
	{/snippet}
</Modal>
