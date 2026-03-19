<script lang="ts">
	/**
	 * InlineRename component - reusable inline text editing with save/cancel
	 * Extracts common renaming logic used across DriveCard, SystemDriveCard, Sidebar
	 */
	import { Check, X } from 'lucide-svelte';

	interface Props {
		value: string;
		onSave: (newValue: string) => void;
		onCancel: () => void;
		placeholder?: string;
		class?: string;
	}

	let props: Props = $props();

	const placeholder = $derived(props.placeholder ?? '');
	const className = $derived(props.class ?? '');

	let inputValue = $state(props.value);
	let previousPropValue = $state(props.value);

	$effect(() => {
		if (props.value !== previousPropValue) {
			inputValue = props.value;
			previousPropValue = props.value;
		}
	});

	function handleSave(): void {
		props.onSave(inputValue.trim());
	}

	function handleKeydown(e: KeyboardEvent): void {
		if (e.key === 'Enter') handleSave();
		if (e.key === 'Escape') props.onCancel();
	}

	function handleFocus(e: FocusEvent): void {
		(e.target as HTMLInputElement).select();
	}

	function handleButtonClick(e: MouseEvent, action: () => void): void {
		e.stopPropagation();
		action();
	}
</script>

<div class="flex h-5 items-center gap-1 {className}">
	<input
		type="text"
		bind:value={inputValue}
		onkeydown={handleKeydown}
		onfocus={handleFocus}
		{placeholder}
		class="box-border h-5 min-w-0 flex-1 rounded border border-border-focus bg-surface-primary px-2 text-xs text-text-primary outline-none"
	/>
	<button
		type="button"
		onclick={(e: MouseEvent) => handleButtonClick(e, handleSave)}
		class="flex h-5 w-5 shrink-0 items-center justify-center rounded text-success transition-colors hover:bg-success/20 hover:text-green-400"
		title="Save"
	>
		<Check size={12} />
	</button>
	<button
		type="button"
		onclick={(e: MouseEvent) => handleButtonClick(e, props.onCancel)}
		class="flex h-5 w-5 shrink-0 items-center justify-center rounded text-text-muted transition-colors hover:bg-danger/20 hover:text-danger"
		title="Cancel"
	>
		<X size={12} />
	</button>
</div>
