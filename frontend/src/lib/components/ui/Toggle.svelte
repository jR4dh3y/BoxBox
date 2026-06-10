<script lang="ts">
	interface Props {
		checked?: boolean;
		disabled?: boolean;
		id?: string;
		label?: string;
		showLabel?: boolean;
		onchange?: (checked: boolean) => void;
	}

	let {
		checked = $bindable(false),
		disabled = false,
		id,
		label,
		showLabel = true,
		onchange
	}: Props = $props();

	function handleChange() {
		checked = !checked;
		onchange?.(checked);
	}
</script>

<label class="inline-flex cursor-pointer items-center gap-2" class:opacity-50={disabled}>
	<button
		type="button"
		role="switch"
		aria-checked={checked}
		aria-label={label || 'Toggle'}
		{disabled}
		{id}
		onclick={handleChange}
		class="relative h-5 w-10 rounded-full transition-colors duration-200 {checked
			? 'bg-accent'
			: 'bg-surface-elevated'}"
	>
		<span
			class="absolute top-0.5 left-0.5 h-4 w-4 rounded-full bg-white transition-transform duration-200 {checked
				? 'translate-x-5'
				: 'translate-x-0'}"
		></span>
	</button>
	{#if label && showLabel}
		<span class="text-sm text-text-primary">{label}</span>
	{/if}
</label>
