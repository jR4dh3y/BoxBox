<script lang="ts">
	import './layout.css';
	import favicon from '$lib/assets/favicon.svg';
	import { authStore } from '$lib/stores/auth.svelte';
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { QueryClient, QueryClientProvider } from '@tanstack/svelte-query';
	import { CONFIG } from '$lib/config';
	import { Spinner, Button } from '$lib/components/ui';
	import { FolderOpen } from 'lucide-svelte';

	let { children } = $props();
	let initialized = $state(false);

	const queryClient = new QueryClient({
		defaultOptions: {
			queries: {
				staleTime: CONFIG.query.staleTimeMs,
				retry: 1
			}
		}
	});

	// Public routes that don't require authentication
	const publicRoutes = ['/login', '/test'];
	const isBrowsePage = $derived(page.url.pathname.startsWith('/browse'));
	const isLoginPage = $derived(page.url.pathname.startsWith('/login'));
	const isAuthenticated = $derived(authStore.isAuthenticated);

	onMount(() => {
		authStore.initialize();
		initialized = true;
	});

	$effect(() => {
		if (!initialized) return;

		const currentPath = page.url.pathname;
		const isPublicRoute = publicRoutes.some((route) => currentPath.startsWith(route));

		if (!isAuthenticated && !isPublicRoute) {
			goto('/login');
		} else if (isAuthenticated && currentPath.startsWith('/login')) {
			goto('/browse');
		}
	});

	async function handleLogout() {
		await authStore.logout();
		goto('/login');
	}
</script>

<svelte:head><link rel="icon" href={favicon} /></svelte:head>

<QueryClientProvider client={queryClient}>
	{#if !initialized}
		<div class="flex min-h-screen items-center justify-center bg-surface-primary">
			<Spinner size="lg" />
		</div>
	{:else if isBrowsePage}
		{@render children()}
	{:else}
		<div class="flex min-h-screen flex-col bg-surface-primary">
			{#if isAuthenticated && !isLoginPage}
				<header class="sticky top-0 z-50 border-b border-border-secondary bg-surface-primary px-4">
					<div class="mx-auto flex h-14 max-w-[1400px] items-center justify-between">
						<a
							href="/browse"
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
				class="flex flex-1 flex-col {isAuthenticated && !isLoginPage
					? 'mx-auto w-full max-w-[1400px] p-6'
					: ''}"
			>
				{@render children()}
			</main>
		</div>
	{/if}
</QueryClientProvider>
