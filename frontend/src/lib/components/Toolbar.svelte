<script lang="ts">
	/**
	 * Toolbar component - navigation buttons and path bar
	 */
	import {
		ChevronLeft,
		ChevronRight,
		ChevronUp,
		Home,
		RefreshCw,
		Settings,
		FolderUp
	} from 'lucide-svelte';

	interface Props {
		pathSegments?: string[];
		canGoBack?: boolean;
		canGoForward?: boolean;
		canGoUp?: boolean;
		onBack?: () => void;
		onForward?: () => void;
		onUp?: () => void;
		onNavigate?: (path: string) => void;
		onRefresh?: () => void;
		onSettings?: () => void;
		onUpload?: () => void;
		uploadDisabled?: boolean;
	}

	let {
		pathSegments = [],
		canGoBack = false,
		canGoForward = false,
		canGoUp = false,
		onBack,
		onForward,
		onUp,
		onNavigate,
		onRefresh,
		onSettings,
		onUpload,
		uploadDisabled = false
	}: Props = $props();

	function buildPath(index: number): string {
		return pathSegments.slice(0, index + 1).join('/');
	}

	function handleSegmentClick(index: number) {
		onNavigate?.(buildPath(index));
	}

	function handleRootClick() {
		onNavigate?.('');
	}

	const navBtnClass =
		'w-7 h-7 flex items-center justify-center bg-transparent border-none rounded text-text-secondary cursor-pointer transition-all duration-100 hover:enabled:bg-surface-elevated hover:enabled:text-text-primary disabled:text-text-disabled disabled:cursor-not-allowed';
</script>

<div
	class="flex items-center gap-2 border-b border-border-secondary bg-surface-primary px-3 py-1.5"
>
	<!-- Navigation buttons -->
	<div class="flex gap-0.5">
		<button type="button" class={navBtnClass} disabled={!canGoBack} onclick={onBack} title="Back">
			<ChevronLeft size={18} />
		</button>
		<button
			type="button"
			class={navBtnClass}
			disabled={!canGoForward}
			onclick={onForward}
			title="Forward"
		>
			<ChevronRight size={18} />
		</button>
		<button type="button" class={navBtnClass} disabled={!canGoUp} onclick={onUp} title="Up">
			<ChevronUp size={18} />
		</button>
	</div>

	<!-- Path bar -->
	<div
		class="flex min-w-0 flex-1 items-center gap-1.5 rounded border border-border-primary bg-surface-secondary px-2 py-1"
	>
		<button
			type="button"
			class="flex h-4.5 w-4.5 shrink-0 cursor-pointer items-center justify-center border-none bg-transparent text-text-secondary hover:text-text-primary"
			onclick={handleRootClick}
			title="Go to root"
		>
			<Home size={14} />
		</button>
		<div class="flex flex-1 items-center gap-1 overflow-hidden">
			{#if pathSegments.length === 0}
				<span class="text-[13px] whitespace-nowrap text-text-primary">This Server</span>
			{:else}
				{#each pathSegments as segment, index (buildPath(index))}
					{#if index > 0}
						<span class="text-xs text-text-muted">/</span>
					{/if}
					{#if index === pathSegments.length - 1}
						<span class="text-[13px] whitespace-nowrap text-text-primary">{segment}</span>
					{:else}
						<button
							type="button"
							class="cursor-pointer border-none bg-transparent p-0 text-[13px] text-text-secondary hover:text-white hover:underline"
							onclick={() => handleSegmentClick(index)}
						>
							{segment}
						</button>
					{/if}
				{/each}
			{/if}
		</div>
	</div>

	<!-- Action buttons -->
	<div class="flex gap-1">
		<button
			type="button"
			class={navBtnClass}
			disabled={uploadDisabled}
			onclick={onUpload}
			title="Upload files"
		>
			<FolderUp size={16} />
		</button>
		<button type="button" class={navBtnClass} onclick={onRefresh} title="Refresh">
			<RefreshCw size={16} />
		</button>
		<button type="button" class={navBtnClass} onclick={onSettings} title="Settings">
			<Settings size={16} />
		</button>
	</div>
</div>
