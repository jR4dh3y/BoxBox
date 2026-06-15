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
		onchange?: (value: string) => void;
	}

	let { value = $bindable(''), options, disabled = false, id, name, onchange }: Props = $props();

	function handleChange(e: Event) {
		const target = e.target as HTMLSelectElement;
		value = target.value;
		onchange?.(value);
	}
</script>

<div class="relative w-full">
	<select
		{value}
		{disabled}
		{id}
		{name}
		onchange={handleChange}
		class="h-8 w-full cursor-pointer appearance-none rounded border border-border-primary bg-surface-secondary px-3 pr-8 text-sm leading-none text-text-primary transition-colors duration-150 focus:border-border-focus focus:outline-none disabled:cursor-not-allowed disabled:opacity-50"
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
