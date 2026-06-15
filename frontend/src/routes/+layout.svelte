<script lang="ts">
	import './layout.css';
	import favicon from '$lib/assets/favicon.svg';
	import { authStore, isAuthenticated } from '$lib/stores/auth';
	import { onDestroy, onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { QueryClient, QueryClientProvider } from '@tanstack/svelte-query';
	import { CONFIG } from '$lib/config';
	import { Spinner, Button } from '$lib/components/ui';
	import { FolderOpen } from 'lucide-svelte';
	import { activeJobs, jobsStore } from '$lib/stores/jobs';
	import { websocketStore } from '$lib/stores/websocket';
	import {
		applyAccentColor,
		resolvedBackgroundImageUrl,
		settingsStore
	} from '$lib/stores/settings';
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
	const publicRoutes = ['/login'];
	const isWorkspacePage = $derived(
		page.url.pathname.startsWith('/browse') || page.url.pathname.startsWith('/settings')
	);
	const isLoginPage = $derived(page.url.pathname.startsWith('/login'));
	const backgroundImageMode = $derived(
		normalizeBackgroundImageMode($settingsStore.backgroundImageMode)
	);
	const backgroundImage = $derived($resolvedBackgroundImageUrl);
	const hasBackgroundImage = $derived(backgroundImage !== null);
	const frostedGlass = $derived(hasBackgroundImage && $settingsStore.frostedGlass);
	const backgroundImageStyle = $derived(
		backgroundImage ? `url(${JSON.stringify(backgroundImage)})` : undefined
	);
	const backgroundStyle = $derived(getWallpaperBackgroundStyle(backgroundImageMode));

	onMount(() => {
		authStore.initialize();
		initialized = true;
	});

	onDestroy(() => {
		websocketStore.disconnect();
	});

	$effect(() => {
		if (!initialized) return;

		const currentPath = page.url.pathname;
		const isPublicRoute = publicRoutes.some((route) => currentPath.startsWith(route));

		if (!$isAuthenticated && !isPublicRoute) {
			goto(resolve('/login'));
		} else if ($isAuthenticated && currentPath.startsWith('/login')) {
			goto(resolve('/browse'));
		}
	});

	$effect(() => {
		if (!initialized) return;

		if ($isAuthenticated) {
			websocketStore.connect();

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
		if (!initialized || !$isAuthenticated) return;

		websocketStore.syncJobSubscriptions($activeJobs.map((job) => job.id));
	});

	$effect(() => {
		applyAccentColor($settingsStore.accentColor);
	});

	async function handleLogout() {
		await authStore.logout();
		goto(resolve('/login'));
	}
</script>

<svelte:head><link rel="icon" href={favicon} /></svelte:head>

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
			{:else if isWorkspacePage}
				{@render children()}
			{:else}
				<div class="flex min-h-screen flex-col bg-surface-primary">
					{#if $isAuthenticated && !isLoginPage}
						<header
							class="sticky top-0 z-50 border-b border-border-secondary bg-surface-primary px-4"
						>
							<div class="mx-auto flex h-14 max-w-[1400px] items-center justify-between">
								<a
									href={resolve('/browse')}
									class="flex items-center gap-2 text-lg font-semibold text-text-primary no-underline hover:text-accent"
								>
									<FolderOpen size={24} class="text-accent" />
									<span>File Manager</span>
								</a>
								<nav class="flex items-center gap-4">
									<Button variant="secondary" size="sm" onclick={handleLogout}>Logout</Button>
								</nav>
							</div>
						</header>
					{/if}
					<main
						class="flex flex-1 flex-col {$isAuthenticated && !isLoginPage
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
