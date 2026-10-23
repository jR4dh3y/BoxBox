<script lang="ts">
	import './layout.css';
	import { authStore } from '#lib/stores/auth.svelte.js';
	import { onDestroy, onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { QueryClient, QueryClientProvider } from '@tanstack/svelte-query';
	import { CONFIG } from '#lib/config.js';
	import Spinner from '#lib/components/ui/Spinner.svelte';
	import Button from '#lib/components/ui/Button.svelte';
	import { AlertTriangle, X } from 'lucide-svelte';
	import { AuthRefreshEvent } from '#lib/api/client.js';
	import { isLocalDesktop } from '#lib/api/desktop.js';
	import { isRemotePath } from '#lib/api/remotePaths.js';
	import { jobsStore } from '#lib/stores/jobs.svelte.js';
	import { refreshFilesWhenJobsFinish } from '#lib/stores/refreshFilesOnJobs.js';
	import { websocketStore } from '#lib/stores/websocket.svelte.js';
	import { settingsStore } from '#lib/stores/settings.svelte.js';
	import { shareWallpaperStore } from '#lib/stores/shareWallpaper.svelte.js';
	import { resolveBackgroundImage, resolveBackgroundImageUrl } from '#lib/utils/appearance.js';
	import { accentTheme } from '#lib/theme/accent.svelte.js';
	import {
		getWallpaperBackgroundStyle,
		normalizeBackgroundImageMode
	} from '#lib/utils/wallpaper.js';
	import { syncShareWallpaperOnLogin } from '#lib/utils/wallpaperSync.js';

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
	const publicRoutes = ['/login', '/s/'];
	const isPublicRoute = $derived(publicRoutes.some((route) => page.url.pathname.startsWith(route)));
	// Full-bleed pages render without the standard page chrome
	const isFullBleedPage = $derived(
		page.url.pathname.startsWith('/browse') || page.url.pathname.startsWith('/s/')
	);
	const isLoginPage = $derived(page.url.pathname.startsWith('/login'));
	// Share pages show the share owner's wallpaper, never the visitor's own.
	const isSharePage = $derived(page.url.pathname.startsWith('/s/'));
	let ownBackgroundImage = $state<string | null>(null);
	const ownBackgroundSource = $derived(settingsStore.current.backgroundImage);

	const backgroundImageMode = $derived(
		normalizeBackgroundImageMode(
			isSharePage ? shareWallpaperStore.current?.mode : settingsStore.current.backgroundImageMode
		)
	);

	const backgroundImage = $derived(
		isSharePage ? (shareWallpaperStore.current?.url ?? null) : ownBackgroundImage
	);

	const hasBackgroundImage = $derived(backgroundImage !== null);
	const accentColor = $derived(settingsStore.current.accentColor);
	const accentWallpaper = $derived(
		settingsStore.current.accentColorSource === 'wallpaper' ? backgroundImage : null
	);

	const frostedGlass = $derived(
		hasBackgroundImage &&
			(isSharePage
				? (shareWallpaperStore.current?.frostedGlass ?? false)
				: settingsStore.current.frostedGlass)
	);

	const backgroundImageStyle = $derived(
		backgroundImage ? `url(${JSON.stringify(backgroundImage)})` : undefined
	);
	const backgroundStyle = $derived(getWallpaperBackgroundStyle(backgroundImageMode));

	onMount(() => {
		window.addEventListener(AuthRefreshEvent.eventName, handleAuthRefresh);
		refreshFilesWhenJobsFinish(queryClient);
		void authStore.initialize().finally(() => {
			initialized = true;
		});

		return () => {
			window.removeEventListener(AuthRefreshEvent.eventName, handleAuthRefresh);
		};
	});

	onDestroy(() => {
		websocketStore.disconnect();
	});

	$effect(() => {
		if (!initialized) return;

		const currentPath = page.url.pathname;

		if (!authStore.isAuthenticated && !isPublicRoute) {
			goto(resolve('login'));
		} else if (authStore.isAuthenticated && currentPath.startsWith('/login')) {
			goto(resolve('browse'));
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

		websocketStore.syncJobSubscriptions(
			jobsStore.active
				.filter((job) => !job.id.startsWith('transfer-') && !isRemotePath(job.id))
				.map((job) => job.id)
		);
	});

	$effect(() => {
		if (!initialized || !authStore.isAuthenticated || !isLocalDesktop()) return;
		const timer = setInterval(() => {
			void jobsStore.loadJobs();
		}, 2000);
		return () => clearInterval(timer);
	});

	$effect(() => {
		return accentTheme.apply(accentColor, accentWallpaper);
	});

	// Show the synchronous result first, then the one that needs a local-image lookup.
	$effect(() => {
		const requested = ownBackgroundSource;
		let cancelled = false;

		ownBackgroundImage = resolveBackgroundImage(requested);
		resolveBackgroundImageUrl(requested)
			.then((url) => {
				if (!cancelled) ownBackgroundImage = url;
			})
			.catch(() => {
				if (!cancelled) ownBackgroundImage = null;
			});

		return () => {
			cancelled = true;
		};
	});

	$effect(() => {
		if (initialized && authStore.isAuthenticated && !isSharePage) syncShareWallpaperOnLogin();
	});

	function handleAuthRefresh(event: Event): void {
		if (!(event instanceof AuthRefreshEvent)) return;

		switch (event.result.kind) {
			case 'succeeded':
				authStore.handleRefreshSuccess();
				return;
			case 'failed':
				authStore.handleRefreshFailure(event.result.error, 'refresh');
				return;
			default: {
				const exhaustive: never = event.result;
				return exhaustive;
			}
		}
	}

	async function handleLogout() {
		if (await authStore.logout()) goto(resolve('login'));
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
			{#if !initialized || (!authStore.isAuthenticated && !isPublicRoute)}
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
									href={resolve('browse')}
									class="flex items-center gap-2 text-lg font-semibold text-text-primary no-underline hover:text-accent"
								>
									<img src="/logo-boxbox-3d.svg" alt="BoxBox" class="h-9 w-auto" />
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
			{#if authStore.authAlert}
				<div
					class="fixed right-4 z-[200] flex w-[calc(100vw-2rem)] max-w-lg items-start gap-3 rounded-lg border px-4 py-3 shadow-lg {isLoginPage
						? 'top-4'
						: 'bottom-11'} {authStore.authAlert.kind === 'terminal'
						? 'border-warning/40 bg-warning/10 text-warning'
						: 'border-danger/30 bg-danger/10 text-danger'}"
					role="alert"
					aria-live="assertive"
				>
					<AlertTriangle size={18} class="mt-0.5 shrink-0" />
					<div class="flex-1 space-y-2">
						<p class="m-0 text-sm">{authStore.authAlert.message}</p>
						{#if authStore.authAlert.kind === 'transient' && authStore.authAlert.source === 'restore'}
							<Button
								type="button"
								variant="secondary"
								size="sm"
								disabled={authStore.isRestoringSession || authStore.isLoading}
								onclick={() => authStore.initialize()}
							>
								{#if authStore.isRestoringSession}
									<Spinner size="sm" />
									<span>Retrying...</span>
								{:else}
									<span>Retry session</span>
								{/if}
							</Button>
						{/if}
					</div>
					<button
						type="button"
						class="shrink-0 rounded p-0.5 transition-colors hover:bg-surface-elevated"
						onclick={() => authStore.clearAuthAlert()}
						aria-label="Dismiss authentication alert"
					>
						<X size={16} />
					</button>
				</div>
			{/if}
		</QueryClientProvider>
	</div>
</div>
