<svelte:options runes={true} />

<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { Button } from '#lib/components/ui/index.js';
	import { AccountPairing } from '#lib/settings/pairing.svelte.js';

	const pairing = new AccountPairing();
	onMount(() => pairing.start());
	onDestroy(() => pairing.dispose());
</script>

<svelte:window onfocus={() => pairing.refresh()} />

<div class="border-t border-border-secondary py-4">
	<div class="flex items-center justify-between gap-3">
		<h3 class="m-0 text-sm font-medium text-text-primary">Paired devices</h3>
		<Button variant="secondary" size="sm" onclick={() => pairing.refresh()}>Refresh</Button>
	</div>
	<p class="mt-2 text-[13px] text-text-secondary">
		Devices authorized to access this account. Pairing does not indicate whether a device is online.
	</p>
	{#if pairing.clients.kind === 'loading'}
		<p class="mt-3 text-[13px] text-text-secondary">Checking paired devices…</p>
	{:else if pairing.clients.kind === 'ready'}
		{#if pairing.clients.clients.length === 0}
			<p class="mt-3 text-[13px] text-text-secondary">No paired devices.</p>
		{:else}
			<ul class="mt-3 list-none divide-y divide-border-secondary p-0">
				{#each pairing.clients.clients as client (client.clientId)}
					<li class="flex items-center justify-between gap-3 py-3">
						<div class="min-w-0">
							<div class="text-[13px] font-medium break-words text-text-primary">
								{client.clientName}
							</div>
							<p class="mt-1 text-xs text-text-secondary">
								Paired {new Date(client.createdAt).toLocaleString()} · Expires {new Date(
									client.expiresAt
								).toLocaleDateString()}
							</p>
						</div>
						<Button
							variant="secondary"
							size="sm"
							disabled={pairing.revoking !== null}
							onclick={() => pairing.revoke(client.clientId)}
							>{pairing.revoking === client.clientId ? 'Revoking…' : 'Revoke access'}</Button
						>
					</li>
				{/each}
			</ul>
		{/if}
	{/if}
	{#if pairing.connectionError}
		<p class="mt-3 text-[13px] text-danger" role="alert">
			{pairing.connectionError}
		</p>
	{/if}
</div>

<div class="mt-3 border-t border-border-secondary py-4">
	<h3 class="m-0 text-sm font-medium text-text-primary">Pair a desktop</h3>
	<p class="mt-2 text-[13px] text-text-secondary">
		Generate a one-time code to connect the BoxBox desktop app to this server. Only share it with a
		device you trust. The code expires after five minutes.
	</p>
	<form
		class="mt-4 flex flex-col items-start gap-3"
		onsubmit={(event) => {
			event.preventDefault();
			void pairing.generate();
		}}
	>
		<label class="flex w-full max-w-xs flex-col gap-1.5 text-[13px] text-text-primary">
			Confirm your password
			<input
				type="password"
				autocomplete="current-password"
				required
				bind:value={pairing.password}
				disabled={pairing.state.kind === 'generating'}
				class="rounded border border-border-primary bg-surface-secondary px-3 py-2 text-sm focus:border-accent focus:outline-none"
			/>
		</label>
		<Button
			type="submit"
			size="sm"
			disabled={pairing.state.kind === 'generating' || !pairing.password}
		>
			{pairing.state.kind === 'generating' ? 'Generating…' : 'Generate pairing code'}
		</Button>
	</form>
	{#if pairing.state.kind === 'ready'}
		<div class="mt-4" role="status">
			<div class="flex flex-wrap items-center gap-3">
				<code class="text-2xl text-text-primary select-all">{pairing.state.pairing.code}</code>
				<Button variant="secondary" size="sm" onclick={() => pairing.copy()}
					>{pairing.copied ? 'Copied' : 'Copy code'}</Button
				>
			</div>
			<p class="mt-2 text-[13px] text-text-secondary">
				Enter this code in the desktop app. Expires at {new Date(
					pairing.state.pairing.expiresAt
				).toLocaleTimeString()}. Generating another code replaces this one.
			</p>
		</div>
	{:else if pairing.state.kind === 'closed'}
		<p class="mt-3 text-[13px] text-text-secondary" role="status">
			This pairing code was used or closed. Paired devices are listed above.
		</p>
	{:else if pairing.state.kind === 'expired'}
		<p class="mt-3 text-[13px] text-text-secondary" role="status">
			This code expired. Confirm your password to generate another.
		</p>
	{/if}
	{#if pairing.error}
		<p class="mt-3 text-[13px] text-danger" role="alert">{pairing.error}</p>
	{/if}
</div>
