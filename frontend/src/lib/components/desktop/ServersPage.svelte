<script lang="ts">
	import { onMount, tick } from 'svelte';
	import { Button, Input, Spinner } from '#lib/components/ui/index.js';
	import { CONFIG } from '#lib/config.js';
	import { ServersController, type ServerEditor } from '#lib/desktop/servers.svelte.js';
	import { createDelayedFlag } from '#lib/utils/delayedFlag.svelte.js';

	let {
		onChanged = () => {},
		onNavigate
	}: { onChanged?: () => void; onNavigate?: (path: string) => void } = $props();
	const controller = new ServersController(undefined, () => onChanged());
	const { servers, action } = controller;
	const showSpinner = createDelayedFlag(
		() => servers.state.kind === 'loading',
		CONFIG.ui.spinnerDelayMs
	);
	const error = $derived(action.error ?? servers.refreshError);

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

<div class="flex h-full min-h-0 flex-col text-[13px]">
	<div class="flex h-10 shrink-0 items-center gap-2 border-b border-border-secondary px-3">
		<p class="m-0 min-w-0 flex-1 truncate">
			{#if error}<span class="text-danger" role="alert">{error}</span>{/if}
		</p>
		<Button
			variant="primary"
			size="sm"
			disabled={action.busy || controller.editor.kind === 'create'}
			onclick={() => void edit({ kind: 'create' })}>Add server</Button
		>
	</div>

	{#if controller.editor.kind !== 'closed'}
		<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
		<form
			class="flex shrink-0 flex-wrap items-center gap-2 border-b border-border-secondary px-3 py-2"
			aria-label={controller.editor.kind === 'update'
				? `Edit ${controller.editor.server.name}`
				: 'Add server'}
			onsubmit={submit}
			onkeydown={formKeydown}
		>
			{#if controller.editor.kind !== 'pair'}
				<div class="w-48 shrink-0">
					<Input
						id="server-name"
						aria-label="Name"
						placeholder="Name"
						required
						autocomplete="off"
						bind:value={controller.draft.name}
					/>
				</div>
				{#if controller.editor.kind === 'create'}
					<div class="min-w-0 flex-1">
						<Input
							aria-label="Server URL"
							placeholder="https://files.example.com"
							required
							autocomplete="off"
							bind:value={controller.draft.url}
						/>
					</div>
				{/if}
			{/if}
			{#if controller.editor.kind !== 'update'}
				<div class="w-40 shrink-0">
					<Input
						id="server-code"
						aria-label="Single-use pairing code"
						placeholder="XXXX-XXXX"
						required
						autocomplete="off"
						bind:value={controller.draft.code}
					/>
				</div>
			{/if}
			<Button type="submit" variant="primary" size="sm" disabled={action.busy}>
				{action.busy
					? 'Saving…'
					: controller.editor.kind === 'update'
						? 'Save name'
						: 'Pair server'}
			</Button>
			<Button
				variant="ghost"
				size="sm"
				disabled={action.busy}
				onclick={() => controller.edit({ kind: 'closed' })}>Discard</Button
			>
		</form>
	{/if}

	<p class="border-b border-border-secondary px-3 py-2 text-text-secondary">
		Enable Connect mode in the server’s settings, then enter its HTTPS URL and single-use code. Edit
		changes the name; pair a new entry to change the URL.
	</p>
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
			<p class="m-0 py-12 text-center text-text-muted">No servers saved</p>
		{:else}
			<table class="w-full table-fixed border-collapse" aria-label="Saved servers">
				<thead>
					<tr class="text-left text-text-secondary">
						<th
							class="sticky top-0 h-8 w-[30%] border-b border-border-primary bg-surface-secondary px-3 font-medium"
							>Name</th
						>
						<th
							class="sticky top-0 h-8 border-b border-border-primary bg-surface-secondary px-3 font-medium"
							>URL</th
						>
						<th
							class="sticky top-0 h-8 w-80 border-b border-border-primary bg-surface-secondary px-3"
							><span class="sr-only">Actions</span></th
						>
					</tr>
				</thead>
				<tbody>
					{#each servers.state.value as server (server.id)}
						{@const status = controller.statuses.get(server.id)}
						{@const confirming = controller.confirmingRemoveId === server.id}
						<tr class="h-9 border-b border-border-secondary hover:bg-surface-secondary">
							<td class="truncate px-3 text-text-primary" title={server.name}>{server.name}</td>
							<td class="px-3 text-text-secondary"
								><p class="truncate" title={server.url}>{server.url}</p>
								{#if status?.kind === 'failed'}<p role="alert" class="text-xs text-danger">
										{status.message}
									</p>
								{:else if status?.kind === 'checking'}<p class="text-xs text-text-muted">
										Checking server…
									</p>
								{:else if status?.kind === 'ready'}<div class="flex flex-wrap gap-2">
										{#each status.paths as root (root.path)}<button
												type="button"
												class="text-xs text-accent underline"
												onclick={() => onNavigate?.(root.path)}>Browse {root.name}</button
											>{/each}
									</div>{/if}
								<p class="text-xs text-text-muted">
									Access expires {new Date(server.expiresAt).toLocaleDateString()}
								</p></td
							>
							<td class="px-3">
								<div class="flex flex-wrap justify-end gap-1.5">
									<Button
										size="sm"
										disabled={action.busy || status?.kind === 'checking'}
										onclick={() => void controller.check(server)}>Retry</Button
									>
									<Button
										variant="primary"
										size="sm"
										disabled={action.busy}
										onclick={() => void edit({ kind: 'pair', server })}>Pair again</Button
									>
									<Button
										variant="secondary"
										size="sm"
										disabled={action.busy}
										onclick={() => void edit({ kind: 'update', server })}>Edit</Button
									>
									<Button
										variant={confirming ? 'danger' : 'secondary'}
										size="sm"
										disabled={action.busy}
										onclick={() => void controller.remove(server)}
									>
										{#if action.isRunning(server.id, 'remove')}
											Removing…
										{:else if confirming}
											Confirm remove
										{:else}
											Remove
										{/if}
									</Button>
								</div>
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		{/if}
	</div>
</div>
