<script lang="ts">
	import { onMount } from 'svelte';
	import { Button, Input } from '#lib/components/ui/index.js';
	import { PairingController } from '#lib/desktop/pairing.svelte.js';

	const controller = new PairingController();
	let now = $state(Date.now());
	const error = $derived(
		controller.action.error ?? controller.status.refreshError ?? controller.clients.refreshError
	);
	const active = $derived(
		controller.status.value?.enabled === true &&
			Date.parse(controller.status.value.expiresAt ?? '') > now
	);
	const code = $derived(active ? controller.grant?.code : null);

	onMount(() => {
		void controller.load();
		const timer = setInterval(() => {
			now = Date.now();
			void controller.load();
		}, 5000);
		return () => clearInterval(timer);
	});
</script>

<div class="space-y-4 px-4 py-3 text-[13px]">
	<p class="text-text-secondary">
		Pair BoxBox Desktop to browse this server alongside your local files. Each device gets its own
		access, which you can revoke here.
	</p>
	{#if error}<p role="alert" class="text-danger">{error}</p>{/if}
	{#if controller.status.state.kind === 'failed'}
		<p role="alert" class="text-danger">{controller.status.state.message}</p>
		<Button size="sm" onclick={() => void controller.load()}>Try again</Button>
	{:else if controller.status.state.kind === 'loading'}
		<p role="status" class="text-text-secondary">Loading connect mode…</p>
	{:else if active}
		<div class="flex flex-wrap items-center gap-4">
			<div class="min-w-0 flex-1">
				<p class="font-medium text-text-primary">Connect mode is open</p>
				{#if code}<p class="my-2 font-mono text-xl tracking-wider text-accent select-all">
						{code}
					</p>{/if}
				<p class="text-text-secondary">
					{code
						? 'Enter this single-use code in BoxBox Desktop. Keep it private.'
						: 'Use the code shown when you enabled connect mode, or close it and generate a new one.'}
				</p>
				<p class="mt-1 text-text-muted">
					Expires {new Date(controller.status.value?.expiresAt ?? '').toLocaleTimeString()}
				</p>
			</div>
			<Button size="sm" disabled={controller.action.busy} onclick={() => void controller.disable()}
				>Close connect mode</Button
			>
		</div>
	{:else}
		<form
			class="flex flex-wrap items-end gap-3"
			onsubmit={(event) => {
				event.preventDefault();
				void controller.enable();
			}}
		>
			<label class="min-w-48 flex-1 space-y-1 text-text-secondary">
				<span>Confirm your server password</span>
				<Input
					type="password"
					autocomplete="current-password"
					required
					bind:value={controller.password}
				/>
			</label>
			<Button type="submit" size="sm" variant="primary" disabled={controller.action.busy}
				>Enable connect mode</Button
			>
		</form>
	{/if}
	<div class="border-t border-border-secondary pt-3">
		<h3 class="mb-2 font-medium text-text-primary">Paired devices</h3>
		{#if controller.clients.state.kind === 'failed'}
			<p role="alert" class="text-danger">{controller.clients.state.message}</p>
		{:else if controller.clients.value?.length === 0}
			<p class="text-text-muted">No devices paired</p>
		{:else}
			{#each controller.clients.value ?? [] as client (client.id)}
				<div class="flex items-center gap-3 border-b border-border-secondary py-2 last:border-b-0">
					<div class="min-w-0 flex-1">
						<p class="truncate text-text-primary">{client.name}</p>
						<p class="text-xs text-text-muted">
							Paired {new Date(client.createdAt).toLocaleString()}
						</p>
					</div>
					<Button
						size="sm"
						disabled={controller.action.busy}
						onclick={() => void controller.revoke(client.id)}>Revoke access</Button
					>
				</div>
			{/each}
		{/if}
	</div>
</div>
