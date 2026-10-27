<script lang="ts">
	import { authStore } from '#lib/stores/auth.svelte.js';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { tick } from 'svelte';
	import { ArrowRight } from 'lucide-svelte';
	import { Spinner } from '#lib/components/ui/index.js';

	let username = $state('');
	let password = $state('');
	let passwordInput = $state<HTMLInputElement>();
	const authBusy = $derived(authStore.isLoading || authStore.isRestoringSession);
	const canSubmit = $derived(!authBusy && username.trim() !== '' && password !== '');
	const fieldClass =
		'h-9 w-full min-w-0 bg-transparent px-3 text-[13px] text-text-primary placeholder:text-text-muted focus:outline-none disabled:opacity-60';

	// The fields stay disabled while a saved session is checked, and a disabled field cannot take focus.
	let autofocused = false;
	function autofocus(input: HTMLInputElement) {
		if (authBusy || autofocused) return;
		autofocused = true;
		input.focus();
	}

	async function handleSubmit(event: Event) {
		event.preventDefault();

		if (!username.trim() || !password) {
			return;
		}

		const success = await authStore.login(username.trim(), password);
		if (success) {
			goto(resolve('browse'));
			return;
		}
		// The fields were disabled while the request ran; hand the password back for a retry.
		await tick();
		passwordInput?.focus();
		passwordInput?.select();
	}

	// An old error says nothing about the next attempt.
	function clearError() {
		if (authStore.error) authStore.clearError();
	}
</script>

<svelte:head>
	<title>Sign in · BoxBox</title>
</svelte:head>

<div class="flex min-h-screen items-center justify-center bg-surface-primary p-4">
	<form class="flex w-[300px] flex-col items-center" onsubmit={handleSubmit}>
		<!-- The animated mark loads after the form; the box keeps its place meanwhile. -->
		<div class="mb-8 aspect-[57/41] h-24">
			{#await import('#lib/components/AnimatedLogo.svelte') then { default: AnimatedLogo }}
				<AnimatedLogo busy={authStore.isLoading} />
			{:catch}
				<img src="/logo-boxbox-3d.svg" alt="BoxBox" class="h-full w-full" />
			{/await}
		</div>

		<!-- One box for both fields, like a desktop sign-in; the arrow submits. -->
		<div
			class="w-full overflow-hidden rounded border bg-surface-secondary {authStore.error
				? 'border-danger'
				: 'border-border-primary focus-within:border-border-focus'}"
		>
			<label for="username" class="sr-only">Username</label>
			<input
				id="username"
				type="text"
				placeholder="Username"
				bind:value={username}
				oninput={clearError}
				autocomplete="username"
				autocapitalize="none"
				spellcheck="false"
				required
				disabled={authBusy}
				class={fieldClass}
				{@attach autofocus}
			/>
			<div class="flex border-t border-border-secondary">
				<label for="password" class="sr-only">Password</label>
				<input
					id="password"
					bind:this={passwordInput}
					type="password"
					placeholder="Password"
					bind:value={password}
					oninput={clearError}
					autocomplete="current-password"
					required
					disabled={authBusy}
					class={fieldClass}
				/>
				<button
					type="submit"
					class="flex w-9 shrink-0 cursor-pointer items-center justify-center text-accent transition-colors hover:bg-surface-elevated disabled:cursor-default disabled:bg-transparent disabled:text-text-muted"
					disabled={!canSubmit}
					aria-label="Sign in"
					title="Sign in"
				>
					{#if authStore.isLoading}
						<Spinner size="sm" />
					{:else}
						<ArrowRight size={16} />
					{/if}
				</button>
			</div>
		</div>

		<!-- The line keeps its height, so an error does not push the form around. -->
		<p class="m-0 mt-3 h-5 text-center text-[13px] text-danger" role="alert">
			{authStore.error ?? ''}
		</p>
	</form>
</div>
