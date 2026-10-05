<script lang="ts">
	/**
	 * Login page component
	 */
	import { authStore } from '$lib/stores/auth.svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { Button, Input, Spinner } from '$lib/components/ui';
	import { X, AlertTriangle } from 'lucide-svelte';

	let username = $state('');
	let password = $state('');

	async function handleSubmit(event: Event) {
		event.preventDefault();

		if (!username.trim() || !password) {
			return;
		}

		const success = await authStore.login(username.trim(), password);
		if (success) {
			goto(resolve('/browse'));
		}
	}

	function clearError() {
		authStore.clearError();
	}
</script>

<svelte:head>
	<title>Login - BoxBox</title>
</svelte:head>

<div class="flex min-h-screen items-center justify-center bg-surface-primary p-4">
	<div
		class="w-full max-w-[400px] rounded-lg border border-border-primary bg-surface-secondary p-8"
	>
		<div class="mb-8 flex flex-col items-center">
			<img src="/logo-boxbox-3d.svg" alt="BoxBox" class="mb-2 h-16 w-auto" />
			<p class="m-0 text-sm text-text-secondary">Sign in to access your files</p>
		</div>

		<form class="flex flex-col gap-5" onsubmit={handleSubmit}>
			{#if authStore.error}
				<div
					class="flex items-center gap-2 rounded border border-danger/30 bg-danger/20 px-4 py-3 text-sm text-danger"
					role="alert"
				>
					<span class="shrink-0"><AlertTriangle size={16} /></span>
					<span class="flex-1">{authStore.error}</span>
					<button
						type="button"
						class="ml-auto flex h-6 w-6 cursor-pointer items-center justify-center rounded border-none bg-transparent p-0 text-xl text-danger transition-colors hover:bg-danger/30"
						onclick={clearError}
						aria-label="Dismiss error"
					>
						<X size={16} />
					</button>
				</div>
			{/if}

			<div class="flex flex-col gap-2">
				<label for="username" class="text-sm font-medium text-text-secondary">Username</label>
				<Input
					type="text"
					id="username"
					bind:value={username}
					placeholder="Enter your username"
					autocomplete="username"
					required
					disabled={authStore.isLoading}
				/>
			</div>

			<div class="flex flex-col gap-2">
				<label for="password" class="text-sm font-medium text-text-secondary">Password</label>
				<Input
					type="password"
					id="password"
					bind:value={password}
					placeholder="Enter your password"
					autocomplete="current-password"
					required
					disabled={authStore.isLoading}
				/>
			</div>

			<Button
				type="submit"
				variant="primary"
				disabled={authStore.isLoading || !username.trim() || !password}
			>
				{#if authStore.isLoading}
					<Spinner size="sm" />
					<span>Signing in...</span>
				{:else}
					<span>Sign In</span>
				{/if}
			</Button>
		</form>
	</div>
</div>
