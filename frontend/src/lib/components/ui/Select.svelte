<script lang="ts">
	import { ChevronDown } from 'lucide-svelte';

	interface Option {
		value: string;
		label: string;
	}

	interface Props {
		value?: string;
		options: Option[];
		disabled?: boolean;
		id?: string;
		name?: string;
		label?: string;
		swatch?: string;
		onchange?: (value: string) => void;
	}

	let {
		value = $bindable(''),
		options,
		disabled = false,
		id,
		name,
		label,
		swatch,
		onchange
	}: Props = $props();

	function handleChange(event: Event & { currentTarget: HTMLSelectElement }) {
		value = event.currentTarget.value;
		onchange?.(value);
	}
</script>

<div class="relative w-full">
	{#if swatch}
		<span
			class="pointer-events-none absolute top-1/2 left-3 z-10 h-3 w-3 -translate-y-1/2 rounded-full"
			style:background-color={swatch}
			aria-hidden="true"
		></span>
	{/if}
	<select
		{value}
		{disabled}
		{id}
		{name}
		aria-label={label}
		onchange={handleChange}
		class="h-8 w-full cursor-pointer appearance-none rounded border border-border-primary bg-surface-primary pr-8 {swatch
			? 'pl-8'
			: 'pl-3'} text-[13px] leading-none text-text-primary transition-colors duration-150 focus:border-border-focus focus:outline-none disabled:cursor-not-allowed disabled:opacity-50"
	>
		{#each options as option (option.value)}
			<option value={option.value}>{option.label}</option>
		{/each}
	</select>
	<ChevronDown
		size={14}
		class="pointer-events-none absolute top-1/2 right-2 -translate-y-1/2 text-text-muted {disabled
			? 'opacity-50'
			: ''}"
	/>
</div>
