<script lang="ts">
	import { authStore } from '#lib/stores/auth.svelte.js';
	import { goto, replaceState } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { onMount, tick } from 'svelte';
	import { ArrowRight } from 'lucide-svelte';
	import { Spinner } from '#lib/components/ui/index.js';
	import { SSO_LOGIN_PATH } from '#lib/api/authConfig.js';
	import { LoginMethodsStore } from '#lib/auth/loginMethods.svelte.js';
	import { ssoErrorMessage } from '#lib/auth/ssoError.js';

	let username = $state('');
	let password = $state('');
	let passwordInput = $state<HTMLInputElement>();
	const loginMethods = new LoginMethodsStore();
	const methods = $derived(loginMethods.state.kind === 'ready' ? loginMethods.state.methods : null);
	const providerName = $derived(
		methods && methods.kind !== 'password' ? methods.providerName : null
	);
	// Read once: a failed single sign-on lands here with a code, which a reload should not replay.
	let ssoError = $state(ssoErrorMessage(page.url.searchParams.get('sso_error')));
	const error = $derived(authStore.error ?? ssoError);

	onMount(() => {
		void loginMethods.load();
		if (page.url.searchParams.has('sso_error')) replaceState(resolve('login'), {});
	});
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
		ssoError = null;
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

		{#if methods === null}
			<!-- Holds the form's place, so a hidden password form never flashes or shifts the page. -->
			<div class="h-[74px] w-full"></div>
		{:else if methods.kind !== 'sso'}
			<!-- One box for both fields, like a desktop sign-in; the arrow submits. -->
			<div
				class="w-full overflow-hidden rounded border bg-surface-secondary {error
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
		{/if}

		{#if providerName !== null}
			{#if methods?.kind === 'password-and-sso'}
				<div class="my-3 flex w-full items-center gap-2 text-[12px] text-text-muted">
					<span class="h-px flex-1 bg-border-secondary"></span>or<span
						class="h-px flex-1 bg-border-secondary"
					></span>
				</div>
			{/if}
			<!-- A link, not a form: a form's redirect to the provider would break the form-action CSP. -->
			<a
				href={SSO_LOGIN_PATH}
				rel="external"
				onclick={clearError}
				aria-disabled={authBusy}
				class="flex h-9 w-full items-center justify-center rounded border border-border-primary bg-surface-secondary text-[13px] text-text-primary transition-colors hover:bg-surface-elevated focus:border-border-focus focus:outline-none {authBusy
					? 'pointer-events-none opacity-60'
					: ''}"
			>
				Sign in with {providerName}
			</a>
		{/if}

		<!-- The line keeps its height, so an error does not push the form around; long ones wrap. -->
		<p class="m-0 mt-3 min-h-5 text-center text-[13px] text-danger" role="alert">
			{error ?? ''}
		</p>
	</form>
</div>
