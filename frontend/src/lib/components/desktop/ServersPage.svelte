<script lang="ts">
	import DeploymentIcon from './DeploymentIcon.svelte';
	import { deploymentIcons } from '#lib/desktop/deploymentIcon.js';
	import { onMount, tick } from 'svelte';
	import { Button, Input, Spinner } from '#lib/components/ui/index.js';
	import { CONFIG } from '#lib/config.js';
	import { ServersController, type ServerEditor } from '#lib/desktop/servers.svelte.js';
	import { createDelayedFlag } from '#lib/utils/delayedFlag.svelte.js';
	import { connectionSummary, connectionView, serverAddress } from '#lib/desktop/connectionView.js';
	import { Pencil, Plus, Trash2 } from 'lucide-svelte';

	let { onChanged = () => {} }: { onChanged?: () => void } = $props();
	const controller = new ServersController(undefined, () => onChanged());
	const { servers, action } = controller;
	const showSpinner = createDelayedFlag(
		() => servers.state.kind === 'loading',
		CONFIG.ui.spinnerDelayMs
	);
	const error = $derived(action.error ?? servers.refreshError);
	const iconButtonClass =
		'flex h-7 w-7 cursor-pointer items-center justify-center rounded text-text-secondary transition-colors disabled:cursor-not-allowed disabled:opacity-50';

	onMount(() => {
		void controller.refresh();
		const timer = setInterval(() => void controller.refresh(), 30_000);
		return () => clearInterval(timer);
	});

	async function edit(editor: ServerEditor): Promise<void> {
		controller.edit(editor);
		await tick();
		document.getElementById(editor.kind === 'pair' ? 'server-code' : 'server-name')?.focus();
	}

	function submit(event: SubmitEvent): void {
		event.preventDefault();
		void controller.save();
	}

	function formKeydown(event: KeyboardEvent): void {
		if (event.key !== 'Escape' || action.busy) return;
		event.preventDefault();
		controller.edit({ kind: 'closed' });
	}
</script>

{#snippet field(id: string, label: string, hint?: string)}
	<label for={id} class="text-text-secondary"
		>{label}{#if hint}<span class="text-text-muted">{` · ${hint}`}</span>{/if}</label
	>
{/snippet}

<div class="flex h-full min-h-0 flex-col gap-3 py-2 text-[13px]">
	<div class="flex items-center justify-between gap-3">
		<p class="m-0 min-w-0 text-text-secondary">Other BoxBox servers whose drives open here.</p>
		{#if controller.editor.kind !== 'create'}
			<Button
				variant="secondary"
				size="sm"
				disabled={action.busy}
				onclick={() => void edit({ kind: 'create' })}><Plus size={14} />Add server</Button
			>
		{/if}
	</div>

	{#if error}<p class="m-0 text-danger" role="alert">{error}</p>{/if}

	{#if controller.editor.kind !== 'closed'}
		{@const editor = controller.editor}
		<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
		<form
			class="flex flex-col gap-3 rounded-lg border border-border-primary bg-surface-secondary p-3"
			aria-label={editor.kind === 'create'
				? 'Add server'
				: editor.kind === 'pair'
					? `Pair ${editor.server.name} again`
					: `Edit ${editor.server.name}`}
			onsubmit={submit}
			onkeydown={formKeydown}
		>
			{#if editor.kind === 'pair'}
				<p class="m-0 text-text-primary">
					Pair <span class="font-medium">{editor.server.name}</span> again
					<span class="text-text-muted">({serverAddress(editor.server.url)})</span>
				</p>
			{:else}
				<div class="flex flex-wrap items-end gap-3">
					<div class="flex flex-col gap-1">
						{@render field('server-icon', 'Icon')}
						<div class="flex h-8 items-center gap-2">
							<DeploymentIcon icon={controller.draft.icon} />
							<select
								id="server-icon"
								class="h-8 rounded border border-border-primary bg-surface-primary px-2"
								bind:value={controller.draft.icon}
								>{#each deploymentIcons as icon (icon.value)}<option value={icon.value}
										>{icon.label}</option
									>{/each}</select
							>
						</div>
					</div>
					<div class="flex min-w-40 flex-1 flex-col gap-1">
						{@render field('server-name', 'Name')}
						<Input
							id="server-name"
							placeholder="Home server"
							required
							autocomplete="off"
							bind:value={controller.draft.name}
						/>
					</div>
				</div>
				{#if editor.kind === 'create'}
					<div class="flex flex-col gap-1">
						{@render field('server-url', 'Address')}
						<Input
							id="server-url"
							placeholder="https://files.example.com"
							required
							autocomplete="off"
							bind:value={controller.draft.url}
						/>
					</div>
				{:else}
					<p class="m-0 text-xs text-text-muted">
						To use a different address, add it as a new server.
					</p>
				{/if}
			{/if}
			{#if editor.kind !== 'update'}
				<div class="flex flex-col gap-1">
					{@render field('server-code', 'Pairing code', 'from that server’s Settings → Account')}
					<div class="w-44">
						<Input
							id="server-code"
							placeholder="XXXX-XXXX"
							required
							autocomplete="off"
							bind:value={controller.draft.code}
						/>
					</div>
				</div>
			{/if}
			<div class="flex justify-end gap-2">
				<Button
					variant="ghost"
					size="sm"
					disabled={action.busy}
					onclick={() => controller.edit({ kind: 'closed' })}>Cancel</Button
				>
				<Button type="submit" variant="primary" size="sm" disabled={action.busy}>
					{action.busy ? 'Saving…' : editor.kind === 'update' ? 'Save' : 'Pair'}
				</Button>
			</div>
		</form>
	{/if}

	<div class="min-h-0 flex-1 overflow-auto">
		{#if servers.state.kind === 'loading'}
			{#if showSpinner.value}
				<div class="flex items-center justify-center gap-2 py-10 text-text-secondary" role="status">
					<Spinner size="sm" />Loading saved servers…
				</div>
			{/if}
		{:else if servers.state.kind === 'failed'}
			<p class="m-0 flex items-center justify-center gap-3 py-10 text-danger" role="alert">
				{servers.state.message}
				<Button variant="secondary" size="sm" onclick={() => void controller.refresh()}
					>Try again</Button
				>
			</p>
		{:else if servers.state.value.length === 0}
			{#if controller.editor.kind !== 'create'}
				<p class="m-0 py-10 text-center text-text-muted">No servers yet.</p>
			{/if}
		{:else}
			<ul class="m-0 list-none divide-y divide-border-secondary p-0" aria-label="Saved servers">
				{#each servers.state.value as server (server.id)}
					{@const view = connectionView(server, controller.statuses.get(server.id))}
					{@const confirming = controller.confirmingRemoveId === server.id}
					<li class="flex items-center gap-3 py-2.5">
						<span
							class="flex h-9 w-9 shrink-0 items-center justify-center rounded bg-surface-elevated text-text-secondary"
							><DeploymentIcon icon={server.icon} size={18} /></span
						>
						<div class="min-w-0 flex-1">
							<div class="flex min-w-0 items-baseline gap-2">
								<span class="truncate font-medium text-text-primary" title={server.name}
									>{server.name}</span
								>
								<span class="min-w-0 truncate text-xs text-text-muted" title={server.url}
									>{serverAddress(server.url)}</span
								>
							</div>
							<p
								class="m-0 mt-0.5 flex items-center gap-1.5 truncate text-xs {view.kind ===
								'connected'
									? 'text-text-secondary'
									: view.kind === 'checking'
										? 'text-text-muted'
										: 'text-warning'}"
								title={view.kind === 'offline' ? view.detail : undefined}
								role={view.kind === 'offline' || view.kind === 'needs-pairing'
									? 'alert'
									: undefined}
							>
								<span
									class="h-1.5 w-1.5 shrink-0 rounded-full {view.kind === 'connected'
										? 'bg-success'
										: view.kind === 'checking'
											? 'bg-text-muted'
											: 'bg-warning'}"
									aria-hidden="true"
								></span>
								<span class="truncate">{connectionSummary(view)}</span>
							</p>
						</div>
						<div class="flex shrink-0 items-center gap-0.5">
							{#if view.kind === 'needs-pairing'}
								<Button
									variant="secondary"
									size="sm"
									disabled={action.busy}
									onclick={() => void edit({ kind: 'pair', server })}>Pair again</Button
								>
							{:else if view.kind === 'offline'}
								<Button
									variant="secondary"
									size="sm"
									disabled={action.busy}
									onclick={() => void controller.check(server)}>Retry</Button
								>
							{/if}
							{#if confirming}
								<Button
									variant="ghost"
									size="sm"
									disabled={action.busy}
									onclick={() => (controller.confirmingRemoveId = null)}>Cancel</Button
								>
								<Button
									variant="danger"
									size="sm"
									disabled={action.busy}
									onclick={() => void controller.remove(server)}
									>{action.isRunning(server.id, 'remove') ? 'Removing…' : 'Remove'}</Button
								>
							{:else}
								<button
									type="button"
									class="{iconButtonClass} hover:enabled:bg-surface-elevated hover:enabled:text-text-primary"
									title="Edit name and icon"
									aria-label="Edit {server.name}"
									disabled={action.busy}
									onclick={() => void edit({ kind: 'update', server })}><Pencil size={15} /></button
								>
								<button
									type="button"
									class="{iconButtonClass} hover:enabled:bg-danger/15 hover:enabled:text-danger"
									title="Remove server"
									aria-label="Remove {server.name}"
									disabled={action.busy}
									onclick={() => void controller.remove(server)}><Trash2 size={15} /></button
								>
							{/if}
						</div>
					</li>
				{/each}
			</ul>
		{/if}
	</div>
</div>
