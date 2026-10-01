<script lang="ts">
	import './layout.css';
	import { authStore } from '$lib/stores/auth.svelte';
	import { onDestroy, onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { QueryClient, QueryClientProvider } from '@tanstack/svelte-query';
	import { CONFIG } from '$lib/config';
	import { Spinner, Button } from '$lib/components/ui';
	import { FolderOpen } from 'lucide-svelte';
	import { jobsStore } from '$lib/stores/jobs.svelte';
	import { websocketStore } from '$lib/stores/websocket.svelte';
	import { settingsStore } from '$lib/stores/settings.svelte';
	import {
		applyAccentColor,
		resolveBackgroundImage,
		resolveBackgroundImageUrl
	} from '$lib/utils/appearance';
	import { getWallpaperBackgroundStyle, normalizeBackgroundImageMode } from '$lib/utils/wallpaper';

	let { children } = $props();
	let initialized = $state(false);
	let authWasActive = false;

	const queryClient = new QueryClient({
		defaultOptions: {
			queries: {
				staleTime: CONFIG.query.staleTimeMs,
				retry: 1
			}
		}
	});

	// Public routes that don't require authentication
	// '/s/' rather than '/s' so the prefix cannot also match '/settings'
	const publicRoutes = ['/login', '/s/'];
	// Full-bleed pages render without the standard page chrome
	const isFullBleedPage = $derived(
		page.url.pathname.startsWith('/browse') ||
			page.url.pathname.startsWith('/settings') ||
			page.url.pathname.startsWith('/s/')
	);
	const isLoginPage = $derived(page.url.pathname.startsWith('/login'));
	const backgroundImageMode = $derived(
		normalizeBackgroundImageMode(settingsStore.current.backgroundImageMode)
	);
	let backgroundImage = $state<string | null>(null);
	const hasBackgroundImage = $derived(backgroundImage !== null);
	const frostedGlass = $derived(hasBackgroundImage && settingsStore.current.frostedGlass);
	const backgroundImageStyle = $derived(
		backgroundImage ? `url(${JSON.stringify(backgroundImage)})` : undefined
	);
	const backgroundStyle = $derived(getWallpaperBackgroundStyle(backgroundImageMode));

	onMount(() => {
		void authStore.initialize().finally(() => {
			initialized = true;
		});
	});

	onDestroy(() => {
		websocketStore.disconnect();
	});

	$effect(() => {
		if (!initialized) return;

		const currentPath = page.url.pathname;
		const isPublicRoute = publicRoutes.some((route) => currentPath.startsWith(route));

		if (!authStore.isAuthenticated && !isPublicRoute) {
			goto(resolve('/login'));
		} else if (authStore.isAuthenticated && currentPath.startsWith('/login')) {
			goto(resolve('/browse'));
		}
	});

	$effect(() => {
		if (!initialized) return;

		if (authStore.isAuthenticated) {
			websocketStore.connect(authStore.isDevelopment);

			if (!authWasActive) {
				authWasActive = true;
				jobsStore.loadJobs();
			}
		} else if (authWasActive) {
			authWasActive = false;
			websocketStore.disconnect();
			jobsStore.reset();
		}
	});

	$effect(() => {
		if (!initialized || !authStore.isAuthenticated) return;

		websocketStore.syncJobSubscriptions(jobsStore.active.map((job) => job.id));
	});

	$effect(() => {
		applyAccentColor(settingsStore.current.accentColor);
	});

	// Show the synchronous result first, then the one that needs a local-image lookup.
	$effect(() => {
		const requested = settingsStore.current.backgroundImage;
		let cancelled = false;

		backgroundImage = resolveBackgroundImage(requested);
		resolveBackgroundImageUrl(requested)
			.then((url) => {
				if (!cancelled) backgroundImage = url;
			})
			.catch(() => {
				if (!cancelled) backgroundImage = null;
			});

		return () => {
			cancelled = true;
		};
	});

	async function handleLogout() {
		await authStore.logout();
		goto(resolve('/login'));
	}
</script>

<div
	class="app-shell"
	data-has-background={hasBackgroundImage ? 'true' : undefined}
	data-background-mode={hasBackgroundImage ? backgroundImageMode : undefined}
	data-frosted-glass={frostedGlass ? 'true' : undefined}
>
	{#if hasBackgroundImage}
		<div
			class="app-personal-background"
			style:background-image={backgroundImageStyle}
			style:background-size={backgroundStyle.size}
			style:background-repeat={backgroundStyle.repeat}
			style:background-position={backgroundStyle.position}
			aria-hidden="true"
		></div>
	{/if}

	<div class="app-content">
		<QueryClientProvider client={queryClient}>
			{#if !initialized}
				<div class="flex min-h-screen items-center justify-center bg-surface-primary">
					<Spinner size="lg" />
				</div>
			{:else if isFullBleedPage}
				{@render children()}
			{:else}
				<div class="flex min-h-screen flex-col bg-surface-primary">
					{#if authStore.isAuthenticated && !authStore.isDevelopment && !isLoginPage}
						<header
							class="sticky top-0 z-50 border-b border-border-secondary bg-surface-primary px-4"
						>
							<div class="mx-auto flex h-14 max-w-[1400px] items-center justify-between">
								<a
									href={resolve('/browse')}
									class="flex items-center gap-2 text-lg font-semibold text-text-primary no-underline hover:text-accent"
								>
									<FolderOpen size={24} class="text-accent" />
									<span>BoxBox</span>
								</a>
								<nav class="flex items-center gap-4">
									<Button variant="secondary" size="sm" onclick={handleLogout}>Logout</Button>
								</nav>
							</div>
						</header>
					{/if}
					<main
						class="flex flex-1 flex-col {authStore.isAuthenticated && !isLoginPage
							? 'mx-auto w-full max-w-[1400px] p-6'
							: ''}"
					>
						{@render children()}
					</main>
				</div>
			{/if}
		</QueryClientProvider>
	</div>
</div>
